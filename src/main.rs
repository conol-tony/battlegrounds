use bevy::prelude::*;
use rand::prelude::*;

// ============ 游戏常量 ============

const CARD_WIDTH: f32 = 100.0;
const CARD_HEIGHT: f32 = 140.0;
const BOARD_Y: f32 = -150.0;
const ENEMY_BOARD_Y: f32 = 200.0;
const SHOP_Y: f32 = -100.0;
const MAX_GOLD: i32 = 10;
const MAX_BOARD_SIZE: usize = 7;
const SHOP_SIZE: usize = 4;

// ============ 游戏状态 ============

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
enum GameState {
    #[default]
    Shop,
    Battle,
    GameOver,
}

// ============ 组件 ============

#[derive(Component, Clone, Debug)]
struct Minion {
    name: String,
    attack: i32,
    health: i32,
    max_health: i32,
    tier: i32,
    minion_type: MinionType,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum MinionType {
    Beast,
    Mech,
    Demon,
    Dragon,
    Pirate,
    Elemental,
    Neutral,
}

#[derive(Component)]
struct Player {
    gold: i32,
    tier: i32,
    health: i32,
}

#[derive(Component)]
struct Enemy {
    health: i32,
    tier: i32,
}

#[derive(Component)]
struct InShop;

#[derive(Component)]
struct OnBoard;

#[derive(Component, Clone, Copy)]
struct BoardSlot(usize);

#[derive(Component)]
struct ShopSlot(usize);

#[derive(Component)]
struct BattleTimer {
    timer: Timer,
    resolved: bool,
}

#[derive(Component)]
struct MainCamera;

#[derive(Component)]
struct UiLabel;

// ============ 资源 ============

#[derive(Resource)]
struct GameData {
    all_minions: Vec<MinionTemplate>,
}

#[derive(Clone)]
struct MinionTemplate {
    name: String,
    attack: i32,
    health: i32,
    tier: i32,
    minion_type: MinionType,
}

// ============ 消息 ============

#[derive(Message)]
struct StartBattle;

#[derive(Message)]
struct EndBattle(bool);

#[derive(Message)]
struct BuyMinion(usize);

#[derive(Message)]
struct SellMinion(Entity);

#[derive(Message)]
struct RefreshShop;

#[derive(Message)]
struct NextTurn;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_state::<GameState>()
        .add_message::<StartBattle>()
        .add_message::<EndBattle>()
        .add_message::<BuyMinion>()
        .add_message::<SellMinion>()
        .add_message::<RefreshShop>()
        .add_message::<NextTurn>()
        .insert_resource(GameData::new())
        .add_systems(Startup, setup)
        .add_systems(OnEnter(GameState::Shop), enter_shop)
        .add_systems(
            Update,
            (
                handle_input,
                handle_messages,
                handle_shop_ui,
            )
                .run_if(in_state(GameState::Shop)),
        )
        .add_systems(
            Update,
            run_battle.run_if(in_state(GameState::Battle)),
        )
        .add_systems(
            Update,
            game_over_ui.run_if(in_state(GameState::GameOver)),
        )
        .run();
}

// ============ 系统 ============

fn setup(mut commands: Commands) {
    commands.spawn((Camera2d, MainCamera));

    commands.spawn((
        Player {
            gold: 3,
            tier: 1,
            health: 40,
        },
        Name::new("Player"),
    ));

    commands.spawn((
        Enemy {
            health: 40,
            tier: 1,
        },
        Name::new("Enemy"),
    ));

    spawn_ui(&mut commands);
}

fn spawn_ui(commands: &mut Commands) {
    // 标题
    commands.spawn((
        Text2d::new("🏰 酒馆战棋 Demo (Bevy 0.18)"),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Transform::from_xyz(0.0, 345.0, 10.0),
        UiLabel,
    ));

    // 帮助文本
    commands.spawn((
        Text2d::new("按 1-4 购买 | 按 R 刷新(1金) | 按 空格 结束回合 | 点击己方随从出售"),
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(Color::srgb(0.6, 0.6, 0.6)),
        Transform::from_xyz(0.0, 312.0, 10.0),
        UiLabel,
    ));

    // 玩家状态
    commands.spawn((
        Text2d::new(""),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(0.3, 0.9, 0.3)),
        Transform::from_xyz(-300.0, 345.0, 10.0),
        UiLabel,
    ));

    // 敌人状态
    commands.spawn((
        Text2d::new(""),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.3, 0.3)),
        Transform::from_xyz(250.0, 345.0, 10.0),
        UiLabel,
    ));

    // 提示信息
    commands.spawn((
        Text2d::new(""),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.9, 0.3)),
        Transform::from_xyz(0.0, -350.0, 10.0),
        UiLabel,
    ));
}

fn enter_shop(mut commands: Commands, mut game_data: ResMut<GameData>) {
    game_data.refresh_shop(&mut commands, 1);
}

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut mw_buy: MessageWriter<BuyMinion>,
    mut mw_refresh: MessageWriter<RefreshShop>,
    mut mw_next: MessageWriter<NextTurn>,
    q_board_minions: Query<(Entity, &Transform, &Minion), (With<OnBoard>, Without<InShop>)>,
    mut mw_sell: MessageWriter<SellMinion>,
) {
    if keys.just_pressed(KeyCode::Space) {
        mw_next.write(NextTurn);
    }

    if keys.just_pressed(KeyCode::KeyR) {
        mw_refresh.write(RefreshShop);
    }

    if keys.just_pressed(KeyCode::Digit1) {
        mw_buy.write(BuyMinion(0));
    }
    if keys.just_pressed(KeyCode::Digit2) {
        mw_buy.write(BuyMinion(1));
    }
    if keys.just_pressed(KeyCode::Digit3) {
        mw_buy.write(BuyMinion(2));
    }
    if keys.just_pressed(KeyCode::Digit4) {
        mw_buy.write(BuyMinion(3));
    }

    // 点击出售
    if mouse.just_pressed(MouseButton::Left) {
        if let Ok((cam, cam_transform)) = cameras.single() {
            if let Ok(window) = windows.single() {
                if let Some(pos) = window.cursor_position() {
                    let window_size = window.size();
                    if let Ok(world_pos) = screen_to_world(cam, cam_transform, pos, window_size) {
                        for (entity, transform, _minion) in q_board_minions.iter() {
                            let dx = (transform.translation.x - world_pos.x).abs();
                            let dy = (transform.translation.y - world_pos.y).abs();
                            if dx < CARD_WIDTH / 2.0 && dy < CARD_HEIGHT / 2.0 {
                                mw_sell.write(SellMinion(entity));
                            }
                        }
                    }
                }
            }
        }
    }
}

fn screen_to_world(
    cam: &Camera,
    cam_transform: &GlobalTransform,
    screen_pos: Vec2,
    window_size: Vec2,
) -> Result<Vec2, bevy::camera::ViewportConversionError> {
    let ndc = (screen_pos / window_size) * 2.0 - Vec2::ONE;
    cam.viewport_to_world_2d(cam_transform, ndc)
}

fn handle_shop_ui(
    player: Query<&Player>,
    enemy: Query<&Enemy>,
    board_minions: Query<(Entity, &Minion, &BoardSlot), (With<OnBoard>, Without<InShop>)>,
    mut ui_texts: Query<(&mut Text2d, &Transform, &UiLabel)>,
) {
    let Ok(player) = player.single() else { return };
    let Ok(enemy) = enemy.single() else { return };

    let board_count = board_minions.iter().count();

    // 更新商店阶段 UI (只在 shop 状态运行)
    for (mut text, transform, _label) in ui_texts.iter_mut() {
        if (transform.translation.x - (-300.0)).abs() < 5.0
            && (transform.translation.y - 345.0).abs() < 5.0
        {
            text.0 = format!(
                "💛HP:{} | 👑T{} | 💰{}/{} | 📦{}/{}",
                player.health, player.tier, player.gold, MAX_GOLD, board_count, MAX_BOARD_SIZE
            );
        }
        if (transform.translation.x - 250.0).abs() < 5.0
            && (transform.translation.y - 345.0).abs() < 5.0
        {
            text.0 = format!("💀 HP:{} | T{}", enemy.health, enemy.tier);
        }
    }
}

fn handle_messages(
    mut commands: Commands,
    mut player: Query<&mut Player>,
    mut enemy: Query<&mut Enemy>,
    mut game_data: ResMut<GameData>,
    shop_items: Query<(Entity, &ShopSlot, &Minion), With<InShop>>,
    board_minions: Query<(Entity, &Minion, &BoardSlot), (With<OnBoard>, Without<InShop>)>,
    mut next_state: ResMut<NextState<GameState>>,
    mut mr_buy: MessageReader<BuyMinion>,
    mut mr_sell: MessageReader<SellMinion>,
    mut mr_refresh: MessageReader<RefreshShop>,
    mut mr_next: MessageReader<NextTurn>,
) {
    let Ok(mut player) = player.single_mut() else { return };
    let Ok(mut enemy) = enemy.single_mut() else { return };

    // 购买
    for BuyMinion(slot) in mr_buy.read() {
        let cost = 3;
        if player.gold >= cost {
            let board_count = board_minions.iter().count();
            if board_count < MAX_BOARD_SIZE {
                let shop_minion = shop_items
                    .iter()
                    .find(|(_, s, _)| s.0 == *slot)
                    .map(|(e, _, m)| (e, m.clone()));

                if let Some((shop_entity, minion)) = shop_minion {
                    commands.entity(shop_entity).despawn();

                    let used_slots: Vec<usize> =
                        board_minions.iter().map(|(_, _, s)| s.0).collect();
                    let free_slot = (0..MAX_BOARD_SIZE).find(|i| !used_slots.contains(i));

                    if let Some(slot_idx) = free_slot {
                        player.gold -= cost;
                        spawn_minion_card(&mut commands, &minion, slot_idx, false);
                    }
                }
            }
        }
    }

    // 出售
    for SellMinion(entity) in mr_sell.read() {
        if let Ok((_, minion, slot)) = board_minions.get(*entity) {
            if slot.0 < 100 {
                let refund = minion.tier;
                player.gold = (player.gold + refund).min(MAX_GOLD);
                commands.entity(*entity).despawn();
            }
        }
    }

    // 刷新
    for _ in mr_refresh.read() {
        if player.gold >= 1 {
            player.gold -= 1;
            for (entity, _, _) in shop_items.iter() {
                commands.entity(entity).despawn();
            }
            game_data.refresh_shop(&mut commands, player.tier);
        }
    }

    // 下一回合 -> 战斗
    for _ in mr_next.read() {
        let board_count = board_minions.iter().count();
        if board_count > 0 {
            // 清除商店
            for (entity, _, _) in shop_items.iter() {
                commands.entity(entity).despawn();
            }
            // 生成敌方
            game_data.spawn_enemy_board(&mut commands, player.tier);

            next_state.set(GameState::Battle);

            commands.spawn((
                BattleTimer {
                    timer: Timer::from_seconds(0.6, TimerMode::Once),
                    resolved: false,
                },
                Name::new("BattleTimer"),
            ));
        }
    }
}

fn run_battle(
    mut commands: Commands,
    time: Res<Time>,
    mut timer_query: Query<(Entity, &mut BattleTimer)>,
    player_minions: Query<(Entity, &Minion, &BoardSlot), (With<OnBoard>, Without<InShop>)>,
    mut mw_end: MessageWriter<EndBattle>,
    mut next_state: ResMut<NextState<GameState>>,
    mut player: Query<&mut Player>,
    mut enemy: Query<&mut Enemy>,
) {
    let Ok((timer_entity, mut battle_timer)) = timer_query.single_mut() else {
        return;
    };

    if battle_timer.resolved {
        return;
    }

    battle_timer.timer.tick(time.delta());

    if battle_timer.timer.just_finished() {
        battle_timer.resolved = true;

        // 计算战斗力
        let mut player_power: i32 = 0;
        let mut enemy_power: i32 = 0;

        for (_entity, minion, slot) in player_minions.iter() {
            if slot.0 < 100 {
                player_power += minion.attack + minion.health;
            } else {
                enemy_power += minion.attack + minion.health;
            }
        }

        let player_won = player_power >= enemy_power;
        let board_count = player_minions
            .iter()
            .filter(|(_, _, s)| s.0 < 100)
            .count() as i32;

        // 结算伤害
        let Ok(mut p) = player.single_mut() else { return };
        let Ok(mut e) = enemy.single_mut() else { return };

        if player_won {
            let damage = p.tier + board_count;
            e.health -= damage;
        } else {
            let enemy_board_count = player_minions
                .iter()
                .filter(|(_, _, s)| s.0 >= 100)
                .count() as i32;
            let damage = e.tier + enemy_board_count;
            p.health -= damage;
        }

        // 清除敌方
        let enemy_entities: Vec<Entity> = player_minions
            .iter()
            .filter(|(_, _, s)| s.0 >= 100)
            .map(|(e, _, _)| e)
            .collect();
        for e in &enemy_entities {
            commands.entity(*e).despawn();
        }

        commands.entity(timer_entity).despawn();

        // 检查游戏结束
        if p.health <= 0 || e.health <= 0 {
            next_state.set(GameState::GameOver);
        } else {
            // 回到商店阶段
            p.gold = MAX_GOLD;
            next_state.set(GameState::Shop);
        }
    }
}

fn game_over_ui(
    player: Query<&Player>,
    enemy: Query<&Enemy>,
    mut ui_texts: Query<(&mut Text2d, &Transform, &UiLabel)>,
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
    game_data: ResMut<GameData>,
) {
    let Ok(player) = player.single() else { return };
    let Ok(enemy) = enemy.single() else { return };

    for (mut text, transform, _label) in ui_texts.iter_mut() {
        if (transform.translation.x - 0.0).abs() < 5.0
            && (transform.translation.y - 345.0).abs() < 5.0
        {
            if player.health <= 0 {
                text.0 = format!(
                    "💀 你输了! 你的HP:{} 敌方HP:{} | 按 R 重新开始 | ESC 退出",
                    player.health, enemy.health
                );
            } else {
                text.0 = format!(
                    "🏆 你赢了! 你的HP:{} 敌方HP:{} | 按 R 重新开始 | ESC 退出",
                    player.health, enemy.health
                );
            }
        }
        // 清除提示
        if (transform.translation.y - (-350.0)).abs() < 5.0 {
            text.0 = "".to_string();
        }
    }

    if keys.just_pressed(KeyCode::KeyR) {
        restart_game(&mut commands, &mut next_state);
    }
}

fn restart_game(commands: &mut Commands, next_state: &mut NextState<GameState>) {
    // Simple approach: just despawn all game entities and recreate
    // We use a two-step approach: tag entities then despawn
    commands.queue(move |world: &mut World| {
        let to_despawn: Vec<Entity> = world
            .query::<Entity>()
            .iter(world)
            .filter(|&e| {
                world.get::<Player>(e).is_some()
                    || world.get::<Enemy>(e).is_some()
                    || world.get::<OnBoard>(e).is_some()
                    || world.get::<InShop>(e).is_some()
                    || world.get::<BattleTimer>(e).is_some()
            })
            .collect();
        for e in to_despawn {
            world.despawn(e);
        }

        world.spawn((
            Player {
                gold: 3,
                tier: 1,
                health: 40,
            },
            Name::new("Player"),
        ));
        world.spawn((
            Enemy {
                health: 40,
                tier: 1,
            },
            Name::new("Enemy"),
        ));
    });

    next_state.set(GameState::Shop);
}

// ============ 辅助函数 ============

fn spawn_minion_card(
    commands: &mut Commands,
    minion: &Minion,
    slot_idx: usize,
    is_enemy: bool,
) -> Entity {
    let slot = if is_enemy { 100 + slot_idx } else { slot_idx };
    let base_x = -300.0 + slot_idx as f32 * 100.0;
    let y = if is_enemy { ENEMY_BOARD_Y } else { BOARD_Y };

    let bg_color = if is_enemy {
        Color::srgb(0.6, 0.15, 0.15)
    } else {
        Color::srgb(0.15, 0.3, 0.6)
    };

    commands
        .spawn((
            Sprite {
                color: bg_color,
                custom_size: Some(Vec2::new(CARD_WIDTH, CARD_HEIGHT)),
                ..default()
            },
            Transform::from_xyz(base_x, y, 5.0),
            minion.clone(),
            BoardSlot(slot),
            OnBoard,
            Name::new(format!("card_{}", minion.name)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text2d::new(format!("{}", minion.name)),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Transform::from_xyz(0.0, 48.0, 1.0),
            ));
            parent.spawn((
                Text2d::new(format!("⚔{}", minion.attack)),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.9, 0.3)),
                Transform::from_xyz(-28.0, -50.0, 1.0),
            ));
            parent.spawn((
                Text2d::new(format!("❤{}", minion.health)),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.25, 0.25)),
                Transform::from_xyz(28.0, -50.0, 1.0),
            ));
            parent.spawn((
                Text2d::new(format!("⭐{}", minion.tier)),
                TextFont {
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.8, 0.3)),
                Transform::from_xyz(0.0, -63.0, 1.0),
            ));
        })
        .id()
}

// ============ GameData 实现 ============

impl GameData {
    fn new() -> Self {
        let all_minions = vec![
            MinionTemplate {
                name: "🐉暴怒龙人".into(),
                attack: 2,
                health: 4,
                tier: 1,
                minion_type: MinionType::Dragon,
            },
            MinionTemplate {
                name: "🐺凶暴狼".into(),
                attack: 3,
                health: 1,
                tier: 1,
                minion_type: MinionType::Beast,
            },
            MinionTemplate {
                name: "🤖机械助手".into(),
                attack: 1,
                health: 2,
                tier: 1,
                minion_type: MinionType::Mech,
            },
            MinionTemplate {
                name: "👹小鬼".into(),
                attack: 2,
                health: 3,
                tier: 1,
                minion_type: MinionType::Demon,
            },
            MinionTemplate {
                name: "🏴‍☠️海盗斥候".into(),
                attack: 3,
                health: 2,
                tier: 2,
                minion_type: MinionType::Pirate,
            },
            MinionTemplate {
                name: "🔥烈焰元素".into(),
                attack: 4,
                health: 2,
                tier: 2,
                minion_type: MinionType::Elemental,
            },
            MinionTemplate {
                name: "🐲幼龙".into(),
                attack: 2,
                health: 5,
                tier: 2,
                minion_type: MinionType::Dragon,
            },
            MinionTemplate {
                name: "🤖回收机甲".into(),
                attack: 1,
                health: 4,
                tier: 2,
                minion_type: MinionType::Mech,
            },
            MinionTemplate {
                name: "👹深渊领主".into(),
                attack: 4,
                health: 4,
                tier: 3,
                minion_type: MinionType::Demon,
            },
            MinionTemplate {
                name: "🐺狼群首领".into(),
                attack: 5,
                health: 5,
                tier: 3,
                minion_type: MinionType::Beast,
            },
            MinionTemplate {
                name: "🏴‍☠️海盗船长".into(),
                attack: 5,
                health: 3,
                tier: 3,
                minion_type: MinionType::Pirate,
            },
            MinionTemplate {
                name: "🔥熔岩巨人".into(),
                attack: 6,
                health: 6,
                tier: 4,
                minion_type: MinionType::Elemental,
            },
            MinionTemplate {
                name: "🐉远古巨龙".into(),
                attack: 7,
                health: 7,
                tier: 5,
                minion_type: MinionType::Dragon,
            },
            MinionTemplate {
                name: "🤖终极兵器".into(),
                attack: 6,
                health: 8,
                tier: 5,
                minion_type: MinionType::Mech,
            },
            MinionTemplate {
                name: "💀死亡之翼".into(),
                attack: 10,
                health: 10,
                tier: 6,
                minion_type: MinionType::Dragon,
            },
        ];

        Self { all_minions }
    }

    fn refresh_shop(&mut self, commands: &mut Commands, player_tier: i32) {
        let mut rng = rand::thread_rng();

        let available: Vec<&MinionTemplate> = self
            .all_minions
            .iter()
            .filter(|m| m.tier <= player_tier)
            .collect();

        if available.is_empty() {
            return;
        }

        let count = available.len().min(SHOP_SIZE);
        let indices: Vec<usize> = rand::seq::index::sample(&mut rng, available.len(), count)
            .into_vec();

        for (slot, &idx) in indices.iter().enumerate() {
            let template = &available[idx];
            let minion = Minion {
                name: template.name.clone(),
                attack: template.attack,
                health: template.health,
                max_health: template.health,
                tier: template.tier,
                minion_type: template.minion_type.clone(),
            };

            let x = -150.0 + slot as f32 * 100.0;
            commands
                .spawn((
                    Sprite {
                        color: Color::srgb(0.12, 0.12, 0.12),
                        custom_size: Some(Vec2::new(CARD_WIDTH, CARD_HEIGHT)),
                        ..default()
                    },
                    Transform::from_xyz(x, SHOP_Y, 5.0),
                    minion.clone(),
                    ShopSlot(slot),
                    InShop,
                    Name::new(format!("shop_{}", slot)),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Text2d::new(format!("{}", minion.name)),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        Transform::from_xyz(0.0, 48.0, 1.0),
                    ));
                    parent.spawn((
                        Text2d::new(format!("⚔{}", minion.attack)),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.9, 0.3)),
                        Transform::from_xyz(-28.0, -50.0, 1.0),
                    ));
                    parent.spawn((
                        Text2d::new(format!("❤{}", minion.health)),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.25, 0.25)),
                        Transform::from_xyz(28.0, -50.0, 1.0),
                    ));
                    parent.spawn((
                        Text2d::new(format!("⭐{}", minion.tier)),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.8, 0.3)),
                        Transform::from_xyz(0.0, -63.0, 1.0),
                    ));
                    parent.spawn((
                        Text2d::new(format!("💰3金")),
                        TextFont {
                            font_size: 10.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.4, 0.9, 0.3)),
                        Transform::from_xyz(0.0, -76.0, 1.0),
                    ));
                });
        }
    }

    fn spawn_enemy_board(&mut self, commands: &mut Commands, player_tier: i32) {
        let mut rng = rand::thread_rng();
        let count = (player_tier as usize + 1).min(MAX_BOARD_SIZE);

        let available: Vec<&MinionTemplate> = self
            .all_minions
            .iter()
            .filter(|m| m.tier <= player_tier)
            .collect();

        if available.is_empty() {
            return;
        }

        for i in 0..count {
            let idx = rng.gen_range(0..available.len());
            let template = &available[idx];

            let minion = Minion {
                name: template.name.clone(),
                attack: template.attack,
                health: template.health,
                max_health: template.health,
                tier: template.tier,
                minion_type: template.minion_type.clone(),
            };

            spawn_minion_card(commands, &minion, i, true);
        }
    }
}