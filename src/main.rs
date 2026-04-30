use bevy::prelude::*;
use rand::prelude::*;
use rand::rngs::StdRng;
use bevy::window::WindowResolution;

const CARD_W: f32 = 80.0;
const CARD_H: f32 = 130.0;
const SHOP_Y: f32 = -280.0;
const BOARD_Y: f32 = -100.0;
const ENEMY_BOARD_Y: f32 = 100.0;
const MAX_GOLD: i32 = 10;
const MAX_BOARD: usize = 7;
const SHOP_SIZE: usize = 4;

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
#[derive(Component)] struct UiLabel;
#[derive(Component)] struct Player { gold: i32, tier: i32, health: i32 }
#[derive(Component)] struct Enemy  { health: i32, tier: i32 }
#[derive(Component)] struct BattleTimer { timer: Timer, resolved: bool }
#[derive(Component)]
struct DamageText { timer: Timer, velocity: Vec2 }
#[derive(Component)]
struct Dying { timer: Timer }
#[derive(Resource)] struct FontHandle(Handle<Font>);
#[derive(States, Clone, Eq, PartialEq, Hash, Debug, Default)]
enum GameState { #[default] Shop, Battle, GameOver }

#[derive(Message)] struct BuyMinion(usize);
#[derive(Message)] struct SellMinion(Entity);
#[derive(Message)] struct RefreshShop;
#[derive(Message)] struct EndTurn;

struct MinionTemplate { name: String, attack: i32, health: i32, tier: i32, race: Race }
#[derive(Resource)] struct GameData { minions: Vec<MinionTemplate>, rng: StdRng }

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
        .add_message::<BuyMinion>()
        .add_message::<SellMinion>()
        .add_message::<RefreshShop>()
        .add_message::<EndTurn>()
        .add_systems(Startup, setup)
        .add_systems(Update, (
            handle_input, handle_messages, update_shop_ui,
        ).run_if(in_state(GameState::Shop)))
        .add_systems(Update, run_battle.run_if(in_state(GameState::Battle)))
        .add_systems(Update, game_over_ui.run_if(in_state(GameState::GameOver)))
        .add_systems(Update, (animate_damage_texts, animate_dying).chain())
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

    // player info (x=-300, y=355)
    commands.spawn((Text2d::new(""), ff(13.), TextColor(Color::WHITE), Transform::from_xyz(-300., 355., 10.), UiLabel));
    // enemy info (x=250, y=355)
    commands.spawn((Text2d::new(""), ff(13.), TextColor(Color::srgb(1., 0.4, 0.4)), Transform::from_xyz(250., 355., 10.), UiLabel));
    // game over title (x=0, y=355)
    commands.spawn((Text2d::new(""), ff(22.), TextColor(Color::WHITE), Transform::from_xyz(0., 355., 10.), UiLabel));
    // game over subtitle (x=0, y=320)
    commands.spawn((Text2d::new(""), ff(14.), TextColor(Color::srgb(0.8, 0.8, 0.8)), Transform::from_xyz(0., 320., 10.), UiLabel));
    // hint bar
    commands.spawn((Text2d::new("按 B 购买 | S 出售选中 | R 刷新(1金) | E/空格 结束回合 | 1-7 选中随从"), ff(11.), TextColor(Color::srgb(0.6, 0.6, 0.6)), Transform::from_xyz(0., -380., 10.), UiLabel));
    // section labels
    commands.spawn((Text2d::new("── 商店 ──"), ff(12.), TextColor(Color::srgb(0.6, 0.6, 0.6)), Transform::from_xyz(0., -220., 10.)));
    commands.spawn((Text2d::new("── 我方战场 ──"), ff(12.), TextColor(Color::srgb(0.4, 0.7, 1.0)), Transform::from_xyz(0., -40., 10.)));
    commands.spawn((Text2d::new("── 敌方战场 ──"), ff(12.), TextColor(Color::srgb(1., 0.4, 0.4)), Transform::from_xyz(0., 160., 10.)));

    // divider
    commands.spawn((Sprite { color: Color::srgb(0.3, 0.3, 0.3), custom_size: Some(Vec2::new(780., 1.)), ..default() }, Transform::from_xyz(0., -245., 2.)));

    let mut gd = GameData::new();
    gd.refresh_shop(&mut commands, 1);
    commands.insert_resource(gd);
}

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut sel: Local<Option<Entity>>,
    q_board: Query<(Entity, &BoardSlot), With<OnBoard>>,
    mut ew_buy: MessageWriter<BuyMinion>,
    mut ew_sell: MessageWriter<SellMinion>,
    mut ew_refresh: MessageWriter<RefreshShop>,
    mut ew_end: MessageWriter<EndTurn>,
    q_shop: Query<&ShopSlot, With<InShop>>,
) {
    for (i, k) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3,
                   KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7]
        .iter().enumerate()
    {
        if keys.just_pressed(*k) { *sel = q_board.iter().find(|(_, s)| s.0 == i).map(|(e, _)| e); }
    }
    if keys.just_pressed(KeyCode::KeyB) {
        if let Some(s) = q_shop.iter().next() { ew_buy.write(BuyMinion(s.0)); }
    }
    if keys.just_pressed(KeyCode::KeyS) {
        if let Some(e) = *sel { ew_sell.write(SellMinion(e)); *sel = None; }
    }
    if keys.just_pressed(KeyCode::KeyR) { ew_refresh.write(RefreshShop); }
    if keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::KeyE) { ew_end.write(EndTurn); }
}

fn update_shop_ui(
    player: Query<&Player>,
    enemy: Query<&Enemy>,
    q_board: Query<&BoardSlot, With<OnBoard>>,
    mut ui: Query<(&mut Text2d, &Transform, &UiLabel)>,
    font: Res<FontHandle>,
) {
    let Ok(player) = player.single() else { return };
    let Ok(enemy) = enemy.single() else { return };
    let bn = q_board.iter().filter(|s| s.0 < 100).count();
    let _ff = |s: f32| TextFont { font: font.0.clone(), font_size: s, ..default() };
    for (mut t, tr, _) in ui.iter_mut() {
        let (tx, ty) = (tr.translation.x, tr.translation.y);
        if (tx + 300.).abs() < 5. && (ty - 355.).abs() < 5. {
            t.0 = format!("💛HP:{}  👑T{}  💰{}/{}  📦{}/{}", player.health, player.tier, player.gold, MAX_GOLD, bn, MAX_BOARD);
        }
        if (tx - 250.).abs() < 5. && (ty - 355.).abs() < 5. {
            t.0 = format!("💀HP:{}  T{}", enemy.health, enemy.tier);
        }
    }
}

fn handle_messages(
    mut commands: Commands,
    mut player: Query<&mut Player>,
    mut game_data: ResMut<GameData>,
    q_shop: Query<(Entity, &ShopSlot, &Minion), With<InShop>>,
    q_board: Query<(Entity, &Minion, &BoardSlot), (With<OnBoard>, Without<InShop>)>,
    mut next_state: ResMut<NextState<GameState>>,
    mut ev_buy: MessageReader<BuyMinion>,
    mut ev_sell: MessageReader<SellMinion>,
    mut ev_refresh: MessageReader<RefreshShop>,
    mut ev_end: MessageReader<EndTurn>,
    font: Res<FontHandle>,
) {
    let Ok(mut player) = player.single_mut() else { return };

    // buy
    for ev in ev_buy.read() {
        let slot = ev.0;
        if player.gold < 3 || q_board.iter().count() >= MAX_BOARD { continue; }
        if let Some((se, m)) = q_shop.iter().find(|(_, s, _)| s.0 == slot).map(|(e, _, m)| (e, m.clone())) {
            commands.entity(se).despawn();
            let used: Vec<usize> = q_board.iter().map(|(_, _, s)| s.0).collect();
            if let Some(i) = (0..MAX_BOARD).find(|i| !used.contains(i)) {
                player.gold -= 3;
                spawn_card(&mut commands, &m, i, false, &font.0);
            }
        }
    }

    // sell
    for ev in ev_sell.read() {
        let e = ev.0;
        if let Ok((_, m, s)) = q_board.get(e) {
            if s.0 < 100 {
                player.gold = (player.gold + m.tier).min(MAX_GOLD);
                commands.entity(e).despawn();
            }
        }
    }

    // refresh
    for _ in ev_refresh.read() {
        if player.gold >= 1 {
            player.gold -= 1;
            for (e, _, _) in q_shop.iter() { commands.entity(e).despawn(); }
            game_data.refresh_shop(&mut commands, player.tier);
        }
    }

    // end turn
    for _ in ev_end.read() {
        if q_board.iter().filter(|(_, _, s)| s.0 < 100).count() == 0 { continue; }
        for (e, _, _) in q_shop.iter() { commands.entity(e).despawn(); }
        game_data.spawn_enemy(&mut commands, player.tier, &font.0);
        next_state.set(GameState::Battle);
        commands.spawn((BattleTimer { timer: Timer::from_seconds(0.6, TimerMode::Once), resolved: false }, Name::new("bt")));
    }
}

fn run_battle(
    mut commands: Commands, time: Res<Time>,
    mut qt: Query<(Entity, &mut BattleTimer)>,
    qm: Query<(Entity, &Minion, &BoardSlot), With<OnBoard>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut player: Query<&mut Player>,
    mut enemy: Query<&mut Enemy>,
) {
    let Ok((te, mut bt)) = qt.single_mut() else { return };
    if bt.resolved { return; }
    bt.timer.tick(time.delta());
    if !bt.timer.just_finished() { return; }
    bt.resolved = true;

    let (mut pp, mut ep): (i32, i32) = (0, 0);
    for (_, m, s) in qm.iter() {
        if s.0 < 100 { pp += m.attack + m.health; } else { ep += m.attack + m.health; }
    }
    let won = pp >= ep;

    let Ok(mut p) = player.single_mut() else { return };
    let Ok(mut e) = enemy.single_mut() else { return };
    let pc = qm.iter().filter(|(_, _, s)| s.0 < 100).count() as i32;
    let ec = qm.iter().filter(|(_, _, s)| s.0 >= 100).count() as i32;
    if won { e.health -= p.tier + pc; } else { p.health -= e.tier + ec; }

    for (eid, _, s) in qm.iter() { if s.0 >= 100 { commands.entity(eid).despawn(); } }
    commands.entity(te).despawn();

    if p.health <= 0 || e.health <= 0 {
        next_state.set(GameState::GameOver);
    } else {
        p.gold = MAX_GOLD;
        next_state.set(GameState::Shop);
    }
}

fn game_over_ui(
    mut player: Query<&mut Player>,
    mut enemy: Query<&mut Enemy>,
    mut ui: Query<(&mut Text2d, &Transform, &UiLabel)>,
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
    mut game_data: ResMut<GameData>,
    q_board: Query<(Entity, &BoardSlot), With<OnBoard>>,
    font: Res<FontHandle>,
) {
    let Ok(player_ref) = player.single() else { return };
    let Ok(enemy_ref) = enemy.single() else { return };
    let ph = player_ref.health;
    let eh = enemy_ref.health;
    let _ff = |s: f32| TextFont { font: font.0.clone(), font_size: s, ..default() };

    for (mut t, tr, _) in ui.iter_mut() {
        let (tx, ty) = (tr.translation.x, tr.translation.y);
        if tx.abs() < 5. && (ty - 355.).abs() < 5. {
            t.0 = if ph <= 0 {
                format!("💀 你输了! 你的HP:{} 敌方HP:{}", ph, eh)
            } else {
                format!("🏆 你赢了! 你的HP:{} 敌方HP:{}", ph, eh)
            };
        }
        if tx.abs() < 5. && (ty - 320.).abs() < 5. {
            t.0 = "按 R 重新开始 | 按 ESC 退出".to_string();
        }
    }

    if keys.just_pressed(KeyCode::KeyR) {
        for (eid, _) in q_board.iter() { commands.entity(eid).despawn(); }
        if let Ok(mut p) = player.single_mut() { *p = Player { gold: 3, tier: 1, health: 40 }; }
        if let Ok(mut e) = enemy.single_mut() { *e = Enemy { health: 40, tier: 1 }; }
        game_data.refresh_shop(&mut commands, 1);
        next_state.set(GameState::Shop);
    }
    if keys.just_pressed(KeyCode::Escape) { std::process::exit(0); }
}

fn spawn_card(commands: &mut Commands, m: &Minion, i: usize, enemy: bool, font: &Handle<Font>) -> Entity {
    let slot = if enemy { 100 + i } else { i };
    let x = -300. + i as f32 * 100.;
    let y = if enemy { ENEMY_BOARD_Y } else { BOARD_Y };
    let color = if enemy { Color::srgb(0.55, 0.12, 0.12) } else { Color::srgb(0.12, 0.28, 0.55) };
    let ff = |s: f32| TextFont { font: font.clone(), font_size: s, ..default() };

    commands.spawn((
        Sprite { color, custom_size: Some(Vec2::new(CARD_W, CARD_H)), ..default() },
        Transform::from_xyz(x, y, 5.), m.clone(), BoardSlot(slot), OnBoard, Name::new(format!("c_{}", m.name)),
    ))
    .with_children(|p| {
        p.spawn((Text2d::new(format!("{}{}", m.race.icon(), m.name)), ff(9.), TextColor(Color::WHITE), Transform::from_xyz(0., 48., 1.)));
        p.spawn((Text2d::new(format!("⚔{}", m.attack)), ff(12.), TextColor(Color::srgb(1., 0.9, 0.3)), Transform::from_xyz(-28., -50., 1.)));
        p.spawn((Text2d::new(format!("❤{}", m.health)), ff(12.), TextColor(Color::srgb(1., 0.25, 0.25)), Transform::from_xyz(28., -50., 1.)));
        p.spawn((Text2d::new(format!("⭐{}", m.tier)), ff(9.), TextColor(Color::srgb(1., 0.8, 0.3)), Transform::from_xyz(0., -63., 1.)));
    }).id()
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

    fn refresh_shop(&mut self, cmds: &mut Commands, tier: i32) {
        let avail: Vec<&MinionTemplate> = self.minions.iter().filter(|t| t.tier <= tier).collect();
        if avail.is_empty() { return; }
        let n = avail.len().min(SHOP_SIZE);
        let idxs: Vec<usize> = rand::seq::index::sample(&mut self.rng, avail.len(), n).into_vec();
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
                    TextFont { font_size: 9., ..default() }, TextColor(Color::WHITE), Transform::from_xyz(0., 48., 1.)));
                p.spawn((Text2d::new(format!("⚔{}", m.attack)),
                    TextFont { font_size: 12., ..default() }, TextColor(Color::srgb(1., 0.9, 0.3)), Transform::from_xyz(-28., -50., 1.)));
                p.spawn((Text2d::new(format!("❤{}", m.health)),
                    TextFont { font_size: 12., ..default() }, TextColor(Color::srgb(1., 0.25, 0.25)), Transform::from_xyz(28., -50., 1.)));
                p.spawn((Text2d::new(format!("⭐{}", m.tier)),
                    TextFont { font_size: 9., ..default() }, TextColor(Color::srgb(1., 0.8, 0.3)), Transform::from_xyz(0., -63., 1.)));
                p.spawn((Text2d::new("💰3金"),
                    TextFont { font_size: 9., ..default() }, TextColor(Color::srgb(0.4, 0.9, 0.3)), Transform::from_xyz(0., -76., 1.)));
            });
        }
    }

    fn spawn_enemy(&mut self, cmds: &mut Commands, tier: i32, font: &Handle<Font>) {
        let avail: Vec<&MinionTemplate> = self.minions.iter().filter(|t| t.tier <= tier).collect();
        if avail.is_empty() { return; }
        let n = (tier as usize + 2).min(MAX_BOARD);
        for i in 0..n {
            let t = &avail[self.rng.gen_range(0..avail.len())];
            let m = Minion { name: t.name.clone(), attack: t.attack, health: t.health, tier: t.tier, race: t.race.clone() };
            spawn_card(cmds, &m, i, true, font);
        }
    }
}

fn animate_damage_texts(
    mut commands: Commands, time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut Text2d, &mut DamageText)>,
) {
    for (e, mut tf, mut text, mut dt) in q.iter_mut() {
        dt.timer.tick(time.delta());
        let t = dt.timer.elapsed_secs() / dt.timer.duration().as_secs_f32();
        tf.translation += (dt.velocity * time.delta_secs()).extend(0.);
        text.0 = text.0.clone(); // keep text
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
