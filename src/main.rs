use bevy::prelude::*;
use rand::prelude::*;
use rand::rngs::StdRng;
use std::collections::HashMap;
use bevy::window::WindowResolution;

const CARD_W: f32 = 80.0;
const CARD_H: f32 = 130.0;
const SHOP_Y: f32 = -280.0;
const BOARD_Y: f32 = -100.0;
const ENEMY_BOARD_Y: f32 = 100.0;
const MAX_GOLD: i32 = 10;
const MAX_BOARD: usize = 7;
const SHOP_SIZE: usize = 4;
const MAX_TIER: i32 = 6;

#[derive(Component, Clone, Debug)]
struct Minion {
    name: String,
    attack: i32,
    health: i32,
    tier: i32,
    race: Race,
}

#[derive(Component, Clone, Debug, PartialEq)]
enum Race {
    Beast, Demon, Dragon, Elemental, Mech, Murloc, Undead, Quilboar, Pirate, Neutral,
}

impl Race {
    fn icon(&self) -> &'static str {
        match self {
            Race::Beast     => "🐾",
            Race::Demon     => "😈",
            Race::Dragon    => "🐉",
            Race::Elemental => "🌊",
            Race::Mech      => "🤖",
            Race::Murloc    => "🐸",
            Race::Undead    => "💀",
            Race::Quilboar  => "🐗",
            Race::Pirate    => "🏴",
            Race::Neutral   => "⚪",
        }
    }
}

#[derive(Component)] struct ShopSlot(usize);
#[derive(Component)] struct BoardSlot(usize);
#[derive(Component)] struct InShop;
#[derive(Component)] struct OnBoard;
#[derive(Component)] struct Frozen;
#[derive(Component)] struct Player { gold: i32, tier: i32, health: i32 }
#[derive(Component)] struct Enemy  { health: i32, tier: i32 }
#[derive(Component)] struct BattleTimer { timer: Timer }
#[derive(Component)]
struct DamageText { timer: Timer, velocity: Vec2 }
#[derive(Component)]
struct Dying { timer: Timer }

#[derive(Component)]
struct CombatStats { attack: i32, health: i32 }

#[derive(Component)]
enum UiRole { PlayerInfo, EnemyInfo, GameOverTitle, GameOverSub, Hint }

#[derive(Resource)] struct FontHandle(Handle<Font>);
#[derive(States, Clone, Eq, PartialEq, Hash, Debug, Default)]
enum GameState { #[default] Shop, Battle, GameOver }

#[derive(Message)] struct BuyMinion(usize);
#[derive(Message)] struct SellMinion(Entity);
#[derive(Message)] struct RefreshShop;
#[derive(Message)] struct EndTurn;
#[derive(Message)] struct UpgradeTavern;
#[derive(Message)] struct ToggleFreeze;

struct MinionTemplate { name: String, attack: i32, health: i32, tier: i32, race: Race }
#[derive(Resource)] struct GameData { minions: Vec<MinionTemplate>, rng: StdRng }
#[derive(Resource)]
struct CardImages { handles: HashMap<String, Handle<Image>> }

#[derive(Resource, Default)]
struct ShopState { frozen: bool }

fn tier_upgrade_cost(tier: i32) -> i32 {
    (6 - tier).max(2)
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "炉石传说：酒馆战棋".into(),
                resolution: WindowResolution::new(800, 800),
                ..default()
            }),
            ..default()
        }))
        .init_state::<GameState>()
        .init_resource::<ShopState>()
        .add_message::<BuyMinion>()
        .add_message::<SellMinion>()
        .add_message::<RefreshShop>()
        .add_message::<EndTurn>()
        .add_message::<UpgradeTavern>()
        .add_message::<ToggleFreeze>()
        .add_systems(Startup, (setup, load_card_images).chain())
        .add_systems(Update, (
            handle_input, handle_messages, update_shop_ui,
        ).run_if(in_state(GameState::Shop)))
        .add_systems(Update, run_battle.run_if(in_state(GameState::Battle)))
        .add_systems(Update, game_over_ui.run_if(in_state(GameState::GameOver)))
        .add_systems(Update, (animate_damage_texts, animate_dying))
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);
    let font = asset_server.load("fonts/zh_font.ttf");
    commands.insert_resource(FontHandle(font.clone()));
    let ff = |s: f32| TextFont { font: font.clone(), font_size: s, ..default() };

    commands.spawn((
        Sprite { color: Color::srgb(0.08, 0.08, 0.12), custom_size: Some(Vec2::new(800., 800.)), ..default() },
        Transform::from_xyz(0., 0., 0.),
    ));

    commands.spawn((Player { gold: 3, tier: 1, health: 40 }, Name::new("player")));
    commands.spawn((Enemy  { health: 40, tier: 1 }, Name::new("enemy")));

    commands.spawn((Text2d::new(""), ff(13.), TextColor(Color::WHITE), Transform::from_xyz(-300., 355., 10.), UiRole::PlayerInfo));
    commands.spawn((Text2d::new(""), ff(13.), TextColor(Color::srgb(1., 0.4, 0.4)), Transform::from_xyz(250., 355., 10.), UiRole::EnemyInfo));
    commands.spawn((Text2d::new(""), ff(22.), TextColor(Color::WHITE), Transform::from_xyz(0., 355., 10.), UiRole::GameOverTitle));
    commands.spawn((Text2d::new(""), ff(14.), TextColor(Color::srgb(0.8, 0.8, 0.8)), Transform::from_xyz(0., 320., 10.), UiRole::GameOverSub));
    commands.spawn((Text2d::new("B 购买 | S 出售 | R 刷新(1金) | T 升级 | F 冻结 | E/空格 结束 | 1-4 选商店 | Q/W 切换格"), ff(10.), TextColor(Color::srgb(0.6, 0.6, 0.6)), Transform::from_xyz(0., -380., 10.), UiRole::Hint));
    commands.spawn((Text2d::new("── 商店 ──"), ff(12.), TextColor(Color::srgb(0.6, 0.6, 0.6)), Transform::from_xyz(0., -220., 10.)));
    commands.spawn((Text2d::new("── 我方战场 ──"), ff(12.), TextColor(Color::srgb(0.4, 0.7, 1.0)), Transform::from_xyz(0., -40., 10.)));
    commands.spawn((Text2d::new("── 敌方战场 ──"), ff(12.), TextColor(Color::srgb(1., 0.4, 0.4)), Transform::from_xyz(0., 160., 10.)));

    commands.spawn((Sprite { color: Color::srgb(0.3, 0.3, 0.3), custom_size: Some(Vec2::new(780., 1.)), ..default() }, Transform::from_xyz(0., -245., 2.)));

    let mut gd = GameData::new();
    gd.refresh_shop(&mut commands, 1, &font);
    commands.insert_resource(gd);
}

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut sel_board: Local<Option<Entity>>,
    mut sel_shop: Local<Option<usize>>,
    q_board: Query<(Entity, &BoardSlot), With<OnBoard>>,
    q_shop: Query<&ShopSlot, With<InShop>>,
    mut ew_buy: MessageWriter<BuyMinion>,
    mut ew_sell: MessageWriter<SellMinion>,
    mut ew_refresh: MessageWriter<RefreshShop>,
    mut ew_end: MessageWriter<EndTurn>,
    mut ew_up: MessageWriter<UpgradeTavern>,
    mut ew_freeze: MessageWriter<ToggleFreeze>,
) {
    // Digit1-4: select shop slot
    for (i, k) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4]
        .iter().enumerate()
    {
        if keys.just_pressed(*k) {
            if q_shop.iter().any(|s| s.0 == i) {
                *sel_shop = Some(i);
            }
        }
    }

    let shop_slots: Vec<usize> = {
        let mut v: Vec<usize> = q_shop.iter().map(|s| s.0).collect();
        v.sort();
        v
    };
    if !shop_slots.is_empty() {
        if keys.just_pressed(KeyCode::KeyQ) {
            let cur = sel_shop.unwrap_or(shop_slots[0]);
            let pos = shop_slots.iter().position(|&s| s == cur).unwrap_or(0);
            let new = if pos == 0 { shop_slots.len() - 1 } else { pos - 1 };
            *sel_shop = Some(shop_slots[new]);
        }
        if keys.just_pressed(KeyCode::KeyW) {
            let cur = sel_shop.unwrap_or(shop_slots[0]);
            let pos = shop_slots.iter().position(|&s| s == cur).unwrap_or(0);
            let new = (pos + 1) % shop_slots.len();
            *sel_shop = Some(shop_slots[new]);
        }
        if sel_shop.is_none() {
            *sel_shop = shop_slots.first().copied();
        }
    } else {
        *sel_shop = None;
    }

    // Digit5-7: select board slot for selling
    for (i, k) in [KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7]
        .iter().enumerate()
    {
        if keys.just_pressed(*k) {
            let slot = i + 4;
            *sel_board = q_board.iter().find(|(_, s)| s.0 == slot).map(|(e, _)| e);
        }
    }

    if keys.just_pressed(KeyCode::KeyB) {
        if let Some(slot) = *sel_shop {
            ew_buy.write(BuyMinion(slot));
        }
    }
    if keys.just_pressed(KeyCode::KeyS) {
        if let Some(e) = *sel_board { ew_sell.write(SellMinion(e)); *sel_board = None; }
    }
    if keys.just_pressed(KeyCode::KeyR) { ew_refresh.write(RefreshShop); }
    if keys.just_pressed(KeyCode::KeyT) { ew_up.write(UpgradeTavern); }
    if keys.just_pressed(KeyCode::KeyF) { ew_freeze.write(ToggleFreeze); }
    if keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::KeyE) { ew_end.write(EndTurn); }
}

fn update_shop_ui(
    player: Query<&Player>,
    enemy: Query<&Enemy>,
    q_board: Query<&BoardSlot, With<OnBoard>>,
    shop_state: Res<ShopState>,
    mut ui: Query<(&mut Text2d, &UiRole)>,
) {
    let Ok(player) = player.single() else { return };
    let Ok(enemy) = enemy.single() else { return };
    let bn = q_board.iter().filter(|s| s.0 < 100).count();
    let up_cost = tier_upgrade_cost(player.tier);
    let frozen_txt = if shop_state.frozen { " ❄冻结" } else { "" };
    let up_txt = if player.tier >= MAX_TIER { "MAX".to_string() } else { format!("升级{}金", up_cost) };

    for (mut t, role) in ui.iter_mut() {
        match role {
            UiRole::PlayerInfo => {
                t.0 = format!("💛HP:{}  👑T{}  💰{}/{}  📦{}/{}  ⬆{}{}",
                    player.health, player.tier, player.gold, MAX_GOLD, bn, MAX_BOARD, up_txt, frozen_txt);
            }
            UiRole::EnemyInfo => {
                t.0 = format!("💀HP:{}  T{}", enemy.health, enemy.tier);
            }
            _ => {}
        }
    }
}

fn handle_messages(
    mut commands: Commands,
    mut player: Query<&mut Player>,
    mut game_data: ResMut<GameData>,
    mut shop_state: ResMut<ShopState>,
    q_shop: Query<(Entity, &ShopSlot, &Minion), With<InShop>>,
    q_board: Query<(Entity, &Minion, &BoardSlot), (With<OnBoard>, Without<InShop>)>,
    mut next_state: ResMut<NextState<GameState>>,
    mut ev_buy: MessageReader<BuyMinion>,
    mut ev_sell: MessageReader<SellMinion>,
    mut ev_refresh: MessageReader<RefreshShop>,
    mut ev_end: MessageReader<EndTurn>,
    mut ev_up: MessageReader<UpgradeTavern>,
    mut ev_freeze: MessageReader<ToggleFreeze>,
    font: Res<FontHandle>,
    card_images: Res<CardImages>,
) {
    let Ok(mut player) = player.single_mut() else { return };

    for ev in ev_buy.read() {
        let slot = ev.0;
        if player.gold < 3 || q_board.iter().count() >= MAX_BOARD { continue; }
        if let Some((se, m)) = q_shop.iter().find(|(_, s, _)| s.0 == slot).map(|(e, _, m)| (e, m.clone())) {
            commands.entity(se).despawn();
            let used: Vec<usize> = q_board.iter().map(|(_, _, s)| s.0).collect();
            if let Some(i) = (0..MAX_BOARD).find(|i| !used.contains(i)) {
                player.gold -= 3;
                spawn_card(&mut commands, &m, i, false, &font.0, &card_images);
            }
        }
    }

    for ev in ev_sell.read() {
        let e = ev.0;
        if let Ok((_, m, s)) = q_board.get(e) {
            if s.0 < 100 {
                player.gold = (player.gold + m.tier).min(MAX_GOLD);
                commands.entity(e).despawn();
            }
        }
    }

    for _ in ev_up.read() {
        if player.tier >= MAX_TIER { continue; }
        let cost = tier_upgrade_cost(player.tier);
        if player.gold >= cost {
            player.gold -= cost;
            player.tier += 1;
        }
    }

    for _ in ev_freeze.read() {
        shop_state.frozen = !shop_state.frozen;
    }

    for _ in ev_refresh.read() {
        if player.gold >= 1 {
            player.gold -= 1;
            for (e, _, _) in q_shop.iter() { commands.entity(e).despawn(); }
            shop_state.frozen = false;
            game_data.refresh_shop(&mut commands, player.tier, &font.0);
        }
    }

    for _ in ev_end.read() {
        if q_board.iter().filter(|(_, _, s)| s.0 < 100).count() == 0 { continue; }
        if !shop_state.frozen {
            for (e, _, _) in q_shop.iter() { commands.entity(e).despawn(); }
        }
        game_data.spawn_enemy(&mut commands, player.tier, &font.0, &card_images);
        next_state.set(GameState::Battle);
        commands.spawn((BattleTimer { timer: Timer::from_seconds(0.6, TimerMode::Repeating) }, Name::new("bt")));
    }
}

fn run_battle(
    mut commands: Commands, time: Res<Time>,
    mut qt: Query<(Entity, &mut BattleTimer)>,
    mut qm: Query<(Entity, &mut CombatStats, &Transform, &BoardSlot), With<OnBoard>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut player: Query<&mut Player>,
    mut enemy: Query<&mut Enemy>,
    font: Res<FontHandle>,
) {
    let Ok((te, mut bt)) = qt.single_mut() else { return };
    bt.timer.tick(time.delta());
    if !bt.timer.just_finished() { return; }

    let mut player_side: Vec<(Entity, i32, Vec3)> = Vec::new();
    let mut enemy_side: Vec<(Entity, i32, Vec3)> = Vec::new();
    for (e, cs, tf, s) in qm.iter() {
        if cs.health <= 0 { continue; }
        if s.0 < 100 {
            player_side.push((e, cs.attack, tf.translation));
        } else {
            enemy_side.push((e, cs.attack, tf.translation));
        }
    }
    player_side.sort_by_key(|x| x.0.index());
    enemy_side.sort_by_key(|x| x.0.index());

    if player_side.is_empty() || enemy_side.is_empty() {
        let won = !player_side.is_empty();
        let Ok(mut p) = player.single_mut() else { return };
        let Ok(mut e) = enemy.single_mut() else { return };

        if won {
            let surv = player_side.len() as i32;
            e.health -= p.tier + surv;
        } else {
            let surv = enemy_side.len() as i32;
            p.health -= e.tier + surv;
        }

        let mut to_despawn_enemy: Vec<Entity> = Vec::new();
        let mut to_clean_player: Vec<Entity> = Vec::new();
        for (eid, _cs, _tf, s) in qm.iter() {
            if s.0 >= 100 { to_despawn_enemy.push(eid); } else { to_clean_player.push(eid); }
        }
        for eid in to_despawn_enemy { commands.entity(eid).despawn(); }
        for eid in to_clean_player { commands.entity(eid).remove::<CombatStats>(); }
        commands.entity(te).despawn();

        if p.health <= 0 || e.health <= 0 {
            next_state.set(GameState::GameOver);
        } else {
            p.gold = MAX_GOLD;
            next_state.set(GameState::Shop);
        }
        return;
    }

    let (pe, p_atk, p_pos) = player_side[0];
    let (ee, e_atk, e_pos) = enemy_side[0];

    if let Ok((_, mut cs, _, _)) = qm.get_mut(pe) { cs.health -= e_atk; }
    if let Ok((_, mut cs, _, _)) = qm.get_mut(ee) { cs.health -= p_atk; }

    let ff = |s: f32| TextFont { font: font.0.clone(), font_size: s, ..default() };
    if e_atk > 0 {
        commands.spawn((
            Text2d::new(format!("-{}", e_atk)), ff(16.),
            TextColor(Color::srgb(1.0, 0.3, 0.3)),
            Transform::from_xyz(p_pos.x, p_pos.y + CARD_H * 0.4, 20.),
            DamageText { timer: Timer::from_seconds(0.6, TimerMode::Once), velocity: Vec2::new(0., 60.) },
        ));
    }
    if p_atk > 0 {
        commands.spawn((
            Text2d::new(format!("-{}", p_atk)), ff(16.),
            TextColor(Color::srgb(1.0, 0.3, 0.3)),
            Transform::from_xyz(e_pos.x, e_pos.y + CARD_H * 0.4, 20.),
            DamageText { timer: Timer::from_seconds(0.6, TimerMode::Once), velocity: Vec2::new(0., 60.) },
        ));
    }

    if let Ok((_, cs, _, _)) = qm.get(pe) {
        if cs.health <= 0 {
            commands.entity(pe).insert(Dying { timer: Timer::from_seconds(0.4, TimerMode::Once) });
        }
    }
    if let Ok((_, cs, _, _)) = qm.get(ee) {
        if cs.health <= 0 {
            commands.entity(ee).insert(Dying { timer: Timer::from_seconds(0.4, TimerMode::Once) });
        }
    }
}

fn game_over_ui(
    mut player: Query<&mut Player>,
    mut enemy: Query<&mut Enemy>,
    mut ui: Query<(&mut Text2d, &UiRole)>,
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
    mut game_data: ResMut<GameData>,
    mut shop_state: ResMut<ShopState>,
    q_board: Query<(Entity, &BoardSlot), With<OnBoard>>,
    font: Res<FontHandle>,
) {
    let Ok(player_ref) = player.single() else { return };
    let Ok(enemy_ref) = enemy.single() else { return };
    let ph = player_ref.health;
    let eh = enemy_ref.health;

    for (mut t, role) in ui.iter_mut() {
        match role {
            UiRole::GameOverTitle => {
                t.0 = if ph <= 0 {
                    format!("💀 你输了! 你的HP:{} 敌方HP:{}", ph, eh)
                } else {
                    format!("🏆 你赢了! 你的HP:{} 敌方HP:{}", ph, eh)
                };
            }
            UiRole::GameOverSub => {
                t.0 = "按 R 重新开始 | 按 ESC 退出".to_string();
            }
            _ => {}
        }
    }

    if keys.just_pressed(KeyCode::KeyR) {
        for (eid, _) in q_board.iter() { commands.entity(eid).despawn(); }
        if let Ok(mut p) = player.single_mut() { *p = Player { gold: 3, tier: 1, health: 40 }; }
        if let Ok(mut e) = enemy.single_mut() { *e = Enemy { health: 40, tier: 1 }; }
        shop_state.frozen = false;
        game_data.refresh_shop(&mut commands, 1, &font.0);
        for (mut t, role) in ui.iter_mut() {
            match role {
                UiRole::GameOverTitle | UiRole::GameOverSub => { t.0 = String::new(); }
                _ => {}
            }
        }
        next_state.set(GameState::Shop);
    }
    if keys.just_pressed(KeyCode::Escape) { std::process::exit(0); }
}

fn spawn_card(commands: &mut Commands, m: &Minion, i: usize, enemy: bool, font: &Handle<Font>, images: &CardImages) -> Entity {
    let slot = if enemy { 100 + i } else { i };
    let x = -300. + i as f32 * 100.;
    let y = if enemy { ENEMY_BOARD_Y } else { BOARD_Y };
    let color = if enemy { Color::srgb(0.55, 0.12, 0.12) } else { Color::srgb(0.12, 0.28, 0.55) };
    let ff = |s: f32| TextFont { font: font.clone(), font_size: s, ..default() };

    let key = card_image_key(&m.name);
    let img = if !key.is_empty() { images.handles.get(key).cloned() } else { None };

    if let Some(tex) = img {
        commands.spawn((
            Sprite { color: Color::WHITE, custom_size: Some(Vec2::new(CARD_W, CARD_H)), image: tex, ..default() },
            Transform::from_xyz(x, y, 5.), m.clone(),
            CombatStats { attack: m.attack, health: m.health },
            BoardSlot(slot), OnBoard, Name::new(format!("c_{}", m.name)),
        ))
        .with_children(|p| {
            p.spawn((Text2d::new(format!("⚔{}", m.attack)), ff(12.), TextColor(Color::srgb(1., 0.9, 0.3)), Transform::from_xyz(-28., -50., 1.)));
            p.spawn((Text2d::new(format!("❤{}", m.health)), ff(12.), TextColor(Color::srgb(1., 0.25, 0.25)), Transform::from_xyz(28., -50., 1.)));
            p.spawn((Text2d::new(format!("⭐{}", m.tier)), ff(9.), TextColor(Color::srgb(1., 0.8, 0.3)), Transform::from_xyz(0., -63., 1.)));
        }).id()
    } else {
        commands.spawn((
            Sprite { color, custom_size: Some(Vec2::new(CARD_W, CARD_H)), ..default() },
            Transform::from_xyz(x, y, 5.), m.clone(),
            CombatStats { attack: m.attack, health: m.health },
            BoardSlot(slot), OnBoard, Name::new(format!("c_{}", m.name)),
        ))
        .with_children(|p| {
            p.spawn((Text2d::new(format!("{}{}", m.race.icon(), m.name)), ff(9.), TextColor(Color::WHITE), Transform::from_xyz(0., 48., 1.)));
            p.spawn((Text2d::new(format!("⚔{}", m.attack)), ff(12.), TextColor(Color::srgb(1., 0.9, 0.3)), Transform::from_xyz(-28., -50., 1.)));
            p.spawn((Text2d::new(format!("❤{}", m.health)), ff(12.), TextColor(Color::srgb(1., 0.25, 0.25)), Transform::from_xyz(28., -50., 1.)));
            p.spawn((Text2d::new(format!("⭐{}", m.tier)), ff(9.), TextColor(Color::srgb(1., 0.8, 0.3)), Transform::from_xyz(0., -63., 1.)));
        }).id()
    }
}

impl GameData {
    fn new() -> Self {
        let minions = vec![
            // ── Tier 1 ──
            MinionTemplate { name: "魔刃豹".into(),         attack: 4, health: 1,  tier: 1, race: Race::Beast },
            MinionTemplate { name: "江河弹跳鱼".into(),     attack: 1, health: 1,  tier: 1, race: Race::Beast },
            MinionTemplate { name: "厄运先知".into(),       attack: 2, health: 1,  tier: 1, race: Race::Demon },
            MinionTemplate { name: "挑食魔犬".into(),       attack: 1, health: 1,  tier: 1, race: Race::Demon },
            MinionTemplate { name: "愤怒编织者".into(),     attack: 1, health: 4,  tier: 1, race: Race::Demon },
            MinionTemplate { name: "血色幸存飞龙".into(),   attack: 3, health: 3,  tier: 1, race: Race::Dragon },
            MinionTemplate { name: "暮光龙崽".into(),       attack: 1, health: 1,  tier: 1, race: Race::Dragon },
            MinionTemplate { name: "蓄势主唱幼龙".into(),   attack: 1, health: 1,  tier: 1, race: Race::Dragon },
            MinionTemplate { name: "爆裂飓风".into(),       attack: 2, health: 1,  tier: 1, race: Race::Elemental },
            MinionTemplate { name: "沙丘土著".into(),       attack: 3, health: 2,  tier: 1, race: Race::Elemental },
            MinionTemplate { name: "吵吵机器人".into(),     attack: 1, health: 2,  tier: 1, race: Race::Mech },
            MinionTemplate { name: "拔线机".into(),         attack: 1, health: 1,  tier: 1, race: Race::Mech },
            MinionTemplate { name: "好斗的斥候".into(),     attack: 3, health: 3,  tier: 1, race: Race::Murloc },
            MinionTemplate { name: "无害的骨颅".into(),     attack: 1, health: 1,  tier: 1, race: Race::Undead },
            MinionTemplate { name: "复活的骑兵".into(),     attack: 2, health: 1,  tier: 1, race: Race::Undead },
            MinionTemplate { name: "剃刀沼泽地卜师".into(), attack: 2, health: 1,  tier: 1, race: Race::Quilboar },
            MinionTemplate { name: "晾膘的游客".into(),     attack: 2, health: 3,  tier: 1, race: Race::Quilboar },
            MinionTemplate { name: "夺金健将".into(),       attack: 1, health: 1,  tier: 1, race: Race::Pirate },
            MinionTemplate { name: "南海卖艺者".into(),     attack: 3, health: 1,  tier: 1, race: Race::Pirate },
            MinionTemplate { name: "贪吃的穴居人".into(),   attack: 2, health: 3,  tier: 1, race: Race::Neutral },
            // ── Tier 2 ──
            MinionTemplate { name: "哼鸣蜂鸟".into(),       attack: 1, health: 4,  tier: 2, race: Race::Beast },
            MinionTemplate { name: "下水道老鼠".into(),     attack: 3, health: 2,  tier: 2, race: Race::Beast },
            MinionTemplate { name: "实验室助理".into(),     attack: 3, health: 4,  tier: 2, race: Race::Demon },
            MinionTemplate { name: "灵魂回溯者".into(),     attack: 4, health: 1,  tier: 2, race: Race::Demon },
            MinionTemplate { name: "烈火飞鱼".into(),       attack: 2, health: 4,  tier: 2, race: Race::Dragon },
            MinionTemplate { name: "贪睡的援护巨龙".into(), attack: 4, health: 3,  tier: 2, race: Race::Dragon },
            MinionTemplate { name: "泰蕾苟萨".into(),       attack: 4, health: 4,  tier: 2, race: Race::Dragon },
            MinionTemplate { name: "火焰投球手".into(),     attack: 4, health: 3,  tier: 2, race: Race::Elemental },
            MinionTemplate { name: "商贩元素".into(),       attack: 3, health: 3,  tier: 2, race: Race::Elemental },
            MinionTemplate { name: "冰雪投球手".into(),     attack: 3, health: 4,  tier: 2, race: Race::Elemental },
            MinionTemplate { name: "星元自动机".into(),     attack: 3, health: 4,  tier: 2, race: Race::Mech },
            MinionTemplate { name: "钢铁猎人".into(),       attack: 2, health: 1,  tier: 2, race: Race::Mech },
            MinionTemplate { name: "通报警告机".into(),     attack: 1, health: 1,  tier: 2, race: Race::Mech },
            MinionTemplate { name: "飞行专家".into(),       attack: 3, health: 4,  tier: 2, race: Race::Murloc },
            MinionTemplate { name: "塔德".into(),           attack: 2, health: 2,  tier: 2, race: Race::Murloc },
            MinionTemplate { name: "巨饿冬鳍鱼人".into(),   attack: 2, health: 5,  tier: 2, race: Race::Murloc },
            MinionTemplate { name: "永恒骑士".into(),       attack: 4, health: 1,  tier: 2, race: Race::Undead },
            MinionTemplate { name: "死亡群居蛛魔".into(),   attack: 1, health: 4,  tier: 2, race: Race::Undead },
            MinionTemplate { name: "古老之魂".into(),       attack: 3, health: 4,  tier: 2, race: Race::Undead },
            MinionTemplate { name: "野猪预言者".into(),     attack: 2, health: 3,  tier: 2, race: Race::Quilboar },
            MinionTemplate { name: "挑衅的船工".into(),     attack: 2, health: 5,  tier: 2, race: Race::Pirate },
            MinionTemplate { name: "白赚赌徒".into(),       attack: 3, health: 3,  tier: 2, race: Race::Pirate },
            MinionTemplate { name: "新锐植物学家".into(),   attack: 3, health: 4,  tier: 2, race: Race::Neutral },
            MinionTemplate { name: "耐心的侦查员".into(),   attack: 1, health: 1,  tier: 2, race: Race::Neutral },
            // ── Tier 3 ──
            MinionTemplate { name: "狡猾的迅猛龙".into(),   attack: 1, health: 3,  tier: 3, race: Race::Beast },
            MinionTemplate { name: "邪能元素".into(),       attack: 3, health: 3,  tier: 3, race: Race::Demon },
            MinionTemplate { name: "吸血地狱犬".into(),     attack: 3, health: 3,  tier: 3, race: Race::Demon },
            MinionTemplate { name: "琥珀卫士".into(),       attack: 3, health: 2,  tier: 3, race: Race::Dragon },
            MinionTemplate { name: "钩牙船长".into(),       attack: 1, health: 4,  tier: 3, race: Race::Dragon },
            MinionTemplate { name: "野火元素".into(),       attack: 6, health: 3,  tier: 3, race: Race::Elemental },
            MinionTemplate { name: "聚积风暴".into(),       attack: 5, health: 1,  tier: 3, race: Race::Elemental },
            MinionTemplate { name: "偏折机器人".into(),     attack: 3, health: 2,  tier: 3, race: Race::Mech },
            MinionTemplate { name: "吵吵模组".into(),       attack: 2, health: 4,  tier: 3, race: Race::Mech },
            MinionTemplate { name: "手风琴机器人".into(),   attack: 3, health: 3,  tier: 3, race: Race::Mech },
            MinionTemplate { name: "拜戈尔格国王".into(),   attack: 2, health: 3,  tier: 3, race: Race::Murloc },
            MinionTemplate { name: "刺豚野猪".into(),       attack: 2, health: 6,  tier: 3, race: Race::Quilboar },
            MinionTemplate { name: "刺头吹笛人".into(),     attack: 5, health: 1,  tier: 3, race: Race::Quilboar },
            MinionTemplate { name: "暗膘爵士乐手".into(),   attack: 2, health: 5,  tier: 3, race: Race::Quilboar },
            MinionTemplate { name: "佩吉·斯特迪伯".into(), attack: 2, health: 1,  tier: 3, race: Race::Pirate },
            MinionTemplate { name: "断手被遗忘者".into(),   attack: 2, health: 1,  tier: 3, race: Race::Undead },
            MinionTemplate { name: "致命的孢子".into(),     attack: 1, health: 1,  tier: 3, race: Race::Neutral },
            // ── Tier 4 ──
            MinionTemplate { name: "香蕉猛击者".into(),     attack: 3, health: 6,  tier: 4, race: Race::Beast },
            MinionTemplate { name: "铁喙猫头鹰".into(),     attack: 5, health: 4,  tier: 4, race: Race::Beast },
            MinionTemplate { name: "舞者达瑞尔".into(),     attack: 5, health: 4,  tier: 4, race: Race::Demon },
            MinionTemplate { name: "火药运输工".into(),     attack: 4, health: 5,  tier: 4, race: Race::Demon },
            MinionTemplate { name: "末日之卵".into(),       attack: 0, health: 5,  tier: 4, race: Race::Dragon },
            MinionTemplate { name: "冲浪的希尔梵".into(),   attack: 4, health: 6,  tier: 4, race: Race::Elemental },
            MinionTemplate { name: "机械剑龙".into(),       attack: 3, health: 5,  tier: 4, race: Race::Mech },
            MinionTemplate { name: "寻宝鱼人".into(),       attack: 4, health: 4,  tier: 4, race: Race::Murloc },
            MinionTemplate { name: "拜戈尔格王后".into(),   attack: 6, health: 3,  tier: 4, race: Race::Murloc },
            MinionTemplate { name: "瘟疫行者".into(),       attack: 4, health: 2,  tier: 4, race: Race::Undead },
            MinionTemplate { name: "过路旅客".into(),       attack: 1, health: 10, tier: 4, race: Race::Neutral },
            MinionTemplate { name: "隧道爆破者".into(),     attack: 3, health: 7,  tier: 4, race: Race::Neutral },
            // ── Tier 5 ──
            MinionTemplate { name: "鼠王".into(),           attack: 4, health: 6,  tier: 5, race: Race::Beast },
            MinionTemplate { name: "刺背恶霸".into(),       attack: 8, health: 2,  tier: 5, race: Race::Beast },
            MinionTemplate { name: "大方的地卜师".into(),   attack: 4, health: 6,  tier: 5, race: Race::Demon },
            MinionTemplate { name: "提克特斯".into(),       attack: 3, health: 6,  tier: 5, race: Race::Demon },
            MinionTemplate { name: "玛里苟斯".into(),       attack: 4, health: 12, tier: 5, race: Race::Dragon },
            MinionTemplate { name: "狂风之翼".into(),       attack: 16, health: 8, tier: 5, race: Race::Dragon },
            MinionTemplate { name: "死神4000型".into(),     attack: 6, health: 2,  tier: 5, race: Race::Mech },
            MinionTemplate { name: "菌菇术士弗洛格尔".into(), attack: 4, health: 8, tier: 5, race: Race::Murloc },
            MinionTemplate { name: "尤朵拉船长".into(),     attack: 10, health: 5, tier: 5, race: Race::Pirate },
            MinionTemplate { name: "布莱恩·铜须".into(),   attack: 2,  health: 4, tier: 5, race: Race::Neutral },
            MinionTemplate { name: "瑞文戴尔男爵".into(),   attack: 1,  health: 7, tier: 5, race: Race::Neutral },
            // ── Tier 6 ──
            MinionTemplate { name: "戈德林大狼".into(),     attack: 8,  health: 8,  tier: 6, race: Race::Beast },
            MinionTemplate { name: "饥饿的魔蝠".into(),     attack: 9,  health: 5,  tier: 6, race: Race::Demon },
            MinionTemplate { name: "卡雷苟斯".into(),       attack: 4,  health: 12, tier: 6, race: Race::Dragon },
            MinionTemplate { name: "死亡之翼".into(),       attack: 10, health: 10, tier: 6, race: Race::Dragon },
            MinionTemplate { name: "小瞎眼".into(),         attack: 8,  health: 8,  tier: 6, race: Race::Elemental },
            MinionTemplate { name: "机械加拉克隆".into(),   attack: 6,  health: 6,  tier: 6, race: Race::Mech },
            MinionTemplate { name: "天空上尉库拉格".into(), attack: 4,  health: 6,  tier: 6, race: Race::Pirate },
            MinionTemplate { name: "缝合怪".into(),         attack: 6,  health: 7,  tier: 6, race: Race::Neutral },
        ];
        Self { minions, rng: StdRng::from_entropy() }
    }

    fn refresh_shop(&mut self, cmds: &mut Commands, tier: i32, font: &Handle<Font>) {
        let avail: Vec<&MinionTemplate> = self.minions.iter().filter(|t| t.tier <= tier).collect();
        if avail.is_empty() { return; }
        let n = avail.len().min(SHOP_SIZE);
        let idxs: Vec<usize> = rand::seq::index::sample(&mut self.rng, avail.len(), n).into_vec();
        let ff = |s: f32| TextFont { font: font.clone(), font_size: s, ..default() };
        for (i, &idx) in idxs.iter().enumerate() {
            let t = &avail[idx];
            let m = Minion { name: t.name.clone(), attack: t.attack, health: t.health, tier: t.tier, race: t.race.clone() };
            let x = -150. + i as f32 * 100.;
            cmds.spawn((
                Sprite { color: Color::srgb(0.10, 0.10, 0.10), custom_size: Some(Vec2::new(CARD_W, CARD_H)), ..default() },
                Transform::from_xyz(x, SHOP_Y, 5.), m.clone(), ShopSlot(i), InShop, Name::new(format!("shop_{}", i)),
            ))
            .with_children(|p| {
                p.spawn((Text2d::new(format!("{}{}", m.race.icon(), m.name)),
                    ff(9.), TextColor(Color::WHITE), Transform::from_xyz(0., 48., 1.)));
                p.spawn((Text2d::new(format!("⚔{}", m.attack)),
                    ff(12.), TextColor(Color::srgb(1., 0.9, 0.3)), Transform::from_xyz(-28., -50., 1.)));
                p.spawn((Text2d::new(format!("❤{}", m.health)),
                    ff(12.), TextColor(Color::srgb(1., 0.25, 0.25)), Transform::from_xyz(28., -50., 1.)));
                p.spawn((Text2d::new(format!("⭐{}", m.tier)),
                    ff(9.), TextColor(Color::srgb(1., 0.8, 0.3)), Transform::from_xyz(0., -63., 1.)));
                p.spawn((Text2d::new("💰3金"),
                    ff(9.), TextColor(Color::srgb(0.4, 0.9, 0.3)), Transform::from_xyz(0., -76., 1.)));
            });
        }
    }

    fn spawn_enemy(&mut self, cmds: &mut Commands, tier: i32, font: &Handle<Font>, images: &CardImages) {
        let avail: Vec<&MinionTemplate> = self.minions.iter().filter(|t| t.tier <= tier).collect();
        if avail.is_empty() { return; }
        let n = (tier as usize + 2).min(MAX_BOARD);
        for i in 0..n {
            let t = &avail[self.rng.gen_range(0..avail.len())];
            let m = Minion { name: t.name.clone(), attack: t.attack, health: t.health, tier: t.tier, race: t.race.clone() };
            spawn_card(cmds, &m, i, true, font, images);
        }
    }
}

fn animate_damage_texts(
    mut commands: Commands, time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut DamageText)>,
) {
    for (e, mut tf, mut dt) in q.iter_mut() {
        dt.timer.tick(time.delta());
        tf.translation += (dt.velocity * time.delta_secs()).extend(0.);
        if dt.timer.just_finished() { commands.entity(e).despawn(); }
    }
}

fn animate_dying(
    mut commands: Commands, time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut Sprite, &mut Dying)>,
) {
    for (e, mut tf, mut sprite, mut dy) in q.iter_mut() {
        dy.timer.tick(time.delta());
        let t = dy.timer.elapsed_secs() / dy.timer.duration().as_secs_f32();
        tf.scale = Vec3::splat(1.0 - t * 0.8);
        sprite.color.set_alpha(1.0 - t);
        if dy.timer.just_finished() { commands.entity(e).despawn(); }
    }
}

fn load_card_images(mut commands: Commands, asset_server: Res<AssetServer>) {
    let mut handles: HashMap<String, Handle<Image>> = HashMap::new();
    let names = [
        "Amber_Guardian","Ancestral_Automaton","Annoy-o-Module","Annoy-o-Tron",
        "Banana_Slamma","Bazaar_Dealer","Brann_Bronzebeard","Crackling_Cyclone",
        "Deflect-o-Bot","Dune_Dweller","Eternal_Knight","Famished_Felbat",
        "Felemental","Freedealing_Gambler","Gluttonous_Trogg","Goldrinn_the_Great_Wolf",
        "Harmless_Bonehead","Humming_Bird","Kalecgos_Arcane_Aspect","King_Bagurgle",
        "Laboratory_Assistant","Malchezaar_Prince_of_Dance","Manasaber",
        "Moonsteel_Juggernaut","Nightbane_Ignited","One-Amalgam_Tour_Group",
        "Prophet_of_the_Boar","Pufferquil","Razorfen_Geomancer","Risen_Rider",
        "Scarlet_Survivor","Scrap_Scraper","Sellemental","Sewer_Rat",
        "Southsea_Busker","Sun-Bacon_Relaxer","Tarecgosa","Titus_Rivendare",
        "Tunnel_Blaster","Wrath_Weaver",
    ];
    for n in names {
        handles.insert(n.to_string(), asset_server.load(format!("cards/{}.png", n)));
    }
    commands.insert_resource(CardImages { handles });
}

fn card_image_key(zh_name: &str) -> &str {
    match zh_name {
        "吵吵机器人" => "Annoy-o-Tron",
        "爆裂飓风" => "Crackling_Cyclone",
        "沙丘土著" => "Dune_Dweller",
        "无害的骨颅" => "Harmless_Bonehead",
        "魔刃豹" => "Manasaber",
        "愤怒编织者" => "Wrath_Weaver",
        "血色幸存飞龙" => "Scarlet_Survivor",
        "晾膘的游客" => "Sun-Bacon_Relaxer",
        "南海卖艺者" => "Southsea_Busker",
        "贪吃的穴居人" => "Gluttonous_Trogg",
        "复活的骑兵" => "Risen_Rider",
        "剃刀沼泽地卜师" => "Razorfen_Geomancer",
        "哼鸣蜂鸟" => "Humming_Bird",
        "下水道老鼠" => "Sewer_Rat",
        "永恒骑士" => "Eternal_Knight",
        "泰蕾苟萨" => "Tarecgosa",
        "商贩元素" => "Sellemental",
        "星元自动机" => "Ancestral_Automaton",
        "白赚赌徒" => "Freedealing_Gambler",
        "实验室助理" => "Laboratory_Assistant",
        "野猪预言者" => "Prophet_of_the_Boar",
        "偏折机器人" => "Deflect-o-Bot",
        "吵吵模组" => "Annoy-o-Module",
        "拜戈尔格国王" => "King_Bagurgle",
        "邪能元素" => "Felemental",
        "刺豚野猪" => "Pufferquil",
        "琥珀卫士" => "Amber_Guardian",
        "香蕉猛击者" => "Banana_Slamma",
        "舞者达瑞尔" => "Malchezaar_Prince_of_Dance",
        "隧道爆破者" => "Tunnel_Blaster",
        "布莱恩·铜须" => "Brann_Bronzebeard",
        "瑞文戴尔男爵" => "Titus_Rivendare",
        "玛里苟斯" => "Kalecgos_Arcane_Aspect",
        "大方的地卜师" => "Bazaar_Dealer",
        "死神4000型" => "Scrap_Scraper",
        "戈德林大狼" => "Goldrinn_the_Great_Wolf",
        "饥饿的魔蝠" => "Famished_Felbat",
        "狂风之翼" => "Nightbane_Ignited",
        "缝合怪" => "One-Amalgam_Tour_Group",
        "机械加拉克隆" => "Moonsteel_Juggernaut",
        _ => "",
    }
}
