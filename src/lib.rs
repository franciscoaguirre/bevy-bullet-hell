#![allow(clippy::type_complexity)]

use std::time::Duration;

use bevy::app::App;
use bevy::log;
use bevy::prelude::*;
use bevy::sprite::{collide_aabb::collide, MaterialMesh2dBundle};
use rand::random;

const BULLET_RADIUS: f32 = 10.;
const PLAYER_DIMENSIONS: Vec2 = Vec2::new(50., 50.);
const PLAYER_MAX_HP: u32 = 100;
const PLAYER_COLOR: Color = Color::WHITE;
const HIT_COLOR: Color = Color::RED;
const HIT_FEEDBACK_SECONDS: f32 = 0.05;
const ENEMY_COLOR: Color = Color::GRAY;
const ENEMY_MAX_HP: u32 = 10;
const ENEMY_DIMENSIONS: Vec2 = Vec2::new(50., 50.);
const SCREEN_DIMENSIONS: Vec2 = Vec2::new(600., 800.);
const AUTO_FIRE: bool = false;

#[derive(Component)]
struct Player;

#[derive(Component)]
struct HitPoints(u32);

#[derive(Component)]
struct Gun {
    cooldown_timer: Timer,
    damage: u32,
}

#[derive(Component)]
struct Bullet;

#[derive(Component, Debug)]
enum Hostility {
    Hostile,
    Friendly,
}

#[derive(Component)]
struct Velocity(f32);

#[derive(Component)]
struct Direction(Vec3);

#[derive(Component)]
struct Damage(u32);

#[derive(Component)]
struct Enemy;

#[derive(Component)]
struct HoverBehaviour {
    upper_limit_base: f32,
    upper_limit_margin: f32,
    lower_limit_base: f32,
    lower_limit_margin: f32,
}

#[derive(Component)]
struct Collider;

#[derive(Event, Default)]
struct CollisionEvent;

#[derive(Event, Default)]
struct HitEvent {
    damage: u32,
}

#[derive(Resource)]
struct HitFeedbackTimer(Timer);

impl Default for HitFeedbackTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(0., TimerMode::Once))
    }
}

#[derive(Event, Default)]
struct GameOverEvent;

#[derive(Resource)]
struct EnemySpawnTimer(Timer);

#[derive(Component)]
struct ScoreText;

#[derive(Component)]
struct GameOverText;

#[derive(Resource, Default)]
struct Score(u32);

#[derive(States, Default, Debug, Clone, Hash, Eq, PartialEq)]
enum AppState {
    #[default]
    Restarting,
    Running,
}

impl Default for EnemySpawnTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(2., TimerMode::Once))
    }
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HitFeedbackTimer>()
            .init_resource::<EnemySpawnTimer>()
            .init_resource::<Score>()
            .add_event::<CollisionEvent>()
            .add_event::<HitEvent>()
            .add_event::<GameOverEvent>()
            .add_state::<AppState>()
            .add_systems(Startup, restart) // Goes instantly to "Running"
            .add_systems(Update, (move_player, shoot, limit_player_bounds)) // Player
            .add_systems(Update, (move_bullets, remove_out_of_bounds_bullets)) // Bullets
            .add_systems(
                Update,
                (
                    spawn_enemies,
                    set_enemies_direction,
                    apply_enemy_velocity,
                    enemy_shots,
                ),
            ) // Enemies
            .add_systems(
                Update,
                (increase_score, player_hit, player_hit_feedback, game_over),
            ) // Event listeners
            .add_systems(Update, restart_button) // UI
            .add_systems(OnEnter(AppState::Restarting), restart)
            .add_systems(OnEnter(AppState::Running), setup)
            .add_systems(OnExit(AppState::Running), teardown)
            .add_systems(
                FixedUpdate,
                (check_for_collisions, check_for_collisions_player),
            );
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2dBundle::default());

    commands.spawn((
        MaterialMesh2dBundle {
            mesh: meshes
                .add(shape::Quad::new(Vec2::new(50., 50.)).into())
                .into(),
            material: materials.add(ColorMaterial::from(PLAYER_COLOR)),
            transform: Transform::from_translation(Vec3::new(0., -350., 0.)),
            ..default()
        },
        Player,
        Gun {
            cooldown_timer: Timer::from_seconds(0.25, TimerMode::Once),
            damage: 10,
        },
        HitPoints(PLAYER_MAX_HP),
        Hostility::Friendly,
        Collider,
    ));

    commands.spawn((
        TextBundle::from_section(
            "0",
            TextStyle {
                font_size: 40.,
                ..default()
            },
        )
        .with_text_alignment(TextAlignment::Center),
        ScoreText,
    ));
}

fn move_player(
    time: Res<Time>,
    input: Res<Input<KeyCode>>,
    mut query: Query<&mut Transform, With<Player>>,
) {
    const SPEED: f32 = 600.0;

    for mut transform in query.iter_mut() {
        let mut direction = Vec3::ZERO;

        if input.pressed(KeyCode::Left) || input.pressed(KeyCode::A) {
            direction += Vec3::new(-1.0, 0.0, 0.0);
        }
        if input.pressed(KeyCode::Right) || input.pressed(KeyCode::D) {
            direction += Vec3::new(1.0, 0.0, 0.0);
        }
        if input.pressed(KeyCode::Up) || input.pressed(KeyCode::W) {
            direction += Vec3::new(0.0, 1.0, 0.0);
        }
        if input.pressed(KeyCode::Down) || input.pressed(KeyCode::S) {
            direction += Vec3::new(0.0, -1.0, 0.0);
        }

        if direction.length() > 0.05 {
            transform.translation += direction.normalize() * time.delta_seconds() * SPEED;
        }
    }
}

fn shoot(
    mut commands: Commands,
    input: Res<Input<KeyCode>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut query: Query<(&Transform, &mut Gun), With<Player>>,
    time: Res<Time>,
) {
    for (transform, mut gun) in query.iter_mut() {
        if gun.cooldown_timer.tick(time.delta()).finished() {
            if input.pressed(KeyCode::Space) || AUTO_FIRE {
                commands.spawn(create_bullet(
                    transform.translation.clone() + Vec3::new(0., 50., 0.),
                    &mut meshes,
                    &mut materials,
                    1000.,
                    gun.damage,
                    false,
                ));
                gun.cooldown_timer.reset();
            }
        }
    }
}

fn create_bullet(
    position: Vec3,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    speed: f32,
    damage: u32,
    is_hostile: bool,
) -> (
    MaterialMesh2dBundle<ColorMaterial>,
    Bullet,
    Velocity,
    Damage,
    Hostility,
) {
    (
        MaterialMesh2dBundle {
            mesh: meshes.add(shape::Circle::new(BULLET_RADIUS).into()).into(),
            material: materials.add(ColorMaterial::from(Color::YELLOW)),
            transform: Transform::from_translation(position),
            ..default()
        },
        Bullet,
        Velocity(speed),
        Damage(damage),
        if is_hostile {
            Hostility::Hostile
        } else {
            Hostility::Friendly
        },
    )
}

fn move_bullets(time: Res<Time>, mut query: Query<(&Velocity, &mut Transform), With<Bullet>>) {
    for (velocity, mut transform) in query.iter_mut() {
        transform.translation += Vec3::new(0., 1., 0.) * time.delta_seconds() * velocity.0;
    }
}

fn remove_out_of_bounds_bullets(
    mut commands: Commands,
    query: Query<(&Transform, Entity), With<Bullet>>,
) {
    for (transform, entity) in query.iter() {
        if transform.translation.y > 400. || transform.translation.y < -400. {
            log::info!(
                "Bullet out of bounds at {:?}. Despawning.",
                transform.translation
            );
            commands.entity(entity).despawn();
        }
    }
}

fn spawn_enemies(
    mut commands: Commands,
    time: Res<Time>,
    mut enemy_spawn_timer: ResMut<EnemySpawnTimer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if enemy_spawn_timer.0.tick(time.delta()).just_finished() {
        let random_x = (random::<f32>() * 600. - 300.) * 0.8; // * 0.8 to not spawn enemies at the very edge
        let spawn_point = Vec3::new(random_x, 400., 0.);
        log::info!(
            "Enemy spawn timer finished. Spawning enemy at {:?}.",
            spawn_point
        );
        commands.spawn((
            MaterialMesh2dBundle {
                mesh: meshes.add(shape::Quad::new(ENEMY_DIMENSIONS).into()).into(),
                material: materials.add(ColorMaterial::from(ENEMY_COLOR)),
                transform: Transform::from_translation(spawn_point),
                ..default()
            },
            Enemy,
            Collider,
            Gun {
                cooldown_timer: Timer::from_seconds(1. + random::<f32>(), TimerMode::Once),
                damage: 10,
            },
            HitPoints(ENEMY_MAX_HP),
            Hostility::Hostile,
            Direction(Vec3::ZERO),
            HoverBehaviour {
                upper_limit_base: 300. + random::<f32>() * 100.,
                upper_limit_margin: 50.,
                lower_limit_base: 200. - random::<f32>() * 100.,
                lower_limit_margin: 50.,
            },
        ));
        enemy_spawn_timer
            .0
            .set_duration(Duration::from_secs_f32(1. + random::<f32>()));
        enemy_spawn_timer.0.reset();
    }
}

fn set_enemies_direction(
    mut query: Query<(&Transform, &mut Direction, &HoverBehaviour), With<Enemy>>,
) {
    for (transform, mut direction, hover_behaviour) in query.iter_mut() {
        if transform.translation.y
            < hover_behaviour.lower_limit_base
                - random::<f32>() * hover_behaviour.lower_limit_margin
        {
            direction.0 = Vec3::new(0., 1., 0.);
        } else if transform.translation.y
            > hover_behaviour.upper_limit_base
                + random::<f32>() * hover_behaviour.upper_limit_margin
        {
            direction.0 = Vec3::new(0., -1., 0.);
        }
    }
}

fn apply_enemy_velocity(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &Direction), With<Enemy>>,
) {
    for (mut transform, direction) in query.iter_mut() {
        transform.translation += direction.0 * time.delta_seconds() * 100.;
    }
}

fn enemy_shots(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(&Transform, &mut Gun), With<Enemy>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (transform, mut gun) in query.iter_mut() {
        if gun.cooldown_timer.tick(time.delta()).just_finished() {
            commands.spawn(create_bullet(
                transform.translation.clone() + Vec3::new(0., -50., 0.),
                &mut meshes,
                &mut materials,
                -500.,
                gun.damage,
                true,
            ));
            gun.cooldown_timer
                .set_duration(Duration::from_secs_f32(1. + random::<f32>()));
            gun.cooldown_timer.reset();
        }
    }
}

fn check_for_collisions(
    mut commands: Commands,
    bullet_query: Query<(Entity, &Transform, &Damage, &Hostility), With<Bullet>>,
    mut enemy_query: Query<(Entity, &Transform, &mut HitPoints), With<Enemy>>,
    mut collision_events: EventWriter<CollisionEvent>,
) {
    for (bullet_entity, bullet_transform, bullet_damage, hostility) in bullet_query.iter() {
        for (enemy_entity, enemy_transform, mut enemy_hp) in enemy_query.iter_mut() {
            // No enemy friendly fire
            if let Hostility::Hostile = hostility {
                break;
            }
            let collision = collide(
                bullet_transform.translation,
                Vec2::new(BULLET_RADIUS, BULLET_RADIUS),
                enemy_transform.translation,
                ENEMY_DIMENSIONS,
            );
            if collision.is_some() {
                log::info!(
                    "Found collision! Bullet at {:?} and enemy at {:?}",
                    bullet_transform.translation,
                    enemy_transform.translation
                );
                collision_events.send_default();
                commands.entity(bullet_entity).despawn();
                enemy_hp.0 -= bullet_damage.0;
                if enemy_hp.0 <= 0 {
                    commands.entity(enemy_entity).despawn();
                }
                break;
            }
        }
    }
}

fn check_for_collisions_player(
    mut commands: Commands,
    bullet_query: Query<(Entity, &Transform, &Damage, &Hostility), With<Bullet>>,
    mut player_query: Query<&Transform, With<Player>>,
    mut hit_events: EventWriter<HitEvent>,
) {
    for (bullet_entity, bullet_transform, bullet_damage, hostility) in bullet_query.iter() {
        for player_transform in player_query.iter_mut() {
            // No friendly fire. Unused right now, but maybe in coop?
            if let Hostility::Friendly = hostility {
                break;
            }
            let collision = collide(
                bullet_transform.translation,
                Vec2::new(BULLET_RADIUS, BULLET_RADIUS),
                player_transform.translation,
                PLAYER_DIMENSIONS,
            );
            if collision.is_some() {
                commands.entity(bullet_entity).despawn();
                hit_events.send(HitEvent {
                    damage: bullet_damage.0,
                });
            }
        }
    }
}

fn player_hit_feedback(
    time: Res<Time>,
    mut hit_feedback_timer: ResMut<HitFeedbackTimer>,
    query: Query<&Handle<ColorMaterial>, With<Player>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if hit_feedback_timer.0.tick(time.delta()).just_finished() {
        for handle in query.iter() {
            let material = materials.get_mut(handle).unwrap();
            material.color = PLAYER_COLOR;
        }
    }
}

fn player_hit(
    mut hit_events: EventReader<HitEvent>,
    mut query: Query<(&mut HitPoints, &Handle<ColorMaterial>), With<Player>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut game_over_events: EventWriter<GameOverEvent>,
    mut hit_feedback_timer: ResMut<HitFeedbackTimer>,
) {
    for event in hit_events.read() {
        for (mut hp, material_handle) in query.iter_mut() {
            hp.0 -= event.damage;
            log::info!("Player was hit, HP is now {:?}", hp.0,);
            if hp.0 <= 0 {
                game_over_events.send_default();
            }
            let player_material = materials.get_mut(material_handle).unwrap();
            player_material.color = HIT_COLOR;
            hit_feedback_timer
                .0
                .set_duration(Duration::from_secs_f32(HIT_FEEDBACK_SECONDS));
            hit_feedback_timer.0.reset();
        }
    }
}

fn increase_score(
    mut events: EventReader<CollisionEvent>,
    mut score: ResMut<Score>,
    mut query: Query<&mut Text, With<ScoreText>>,
) {
    for _ in events.read() {
        score.0 += 10;
        for mut text in query.iter_mut() {
            text.sections[0].value = score.0.to_string();
        }
    }
}

fn game_over(
    mut commands: Commands,
    mut events: EventReader<GameOverEvent>,
    player_query: Query<Entity, With<Player>>,
    score_text_query: Query<Entity, With<ScoreText>>,
) {
    for _ in events.read() {
        for player_entity in player_query.iter() {
            for score_text_entity in score_text_query.iter() {
                commands.entity(player_entity).despawn();
                log::info!("Player's HP reached 0, the player has died!");

                commands.entity(score_text_entity).despawn();

                commands.spawn((
                    TextBundle::from_section(
                        "Game over",
                        TextStyle {
                            font_size: 100.,
                            ..default()
                        },
                    ),
                    GameOverText,
                ));

                commands
                    .spawn(NodeBundle {
                        style: Style {
                            width: Val::Percent(100.),
                            height: Val::Percent(100.),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        ..default()
                    })
                    .with_children(|parent| {
                        parent
                            .spawn(ButtonBundle {
                                style: Style {
                                    width: Val::Px(150.),
                                    height: Val::Px(65.),
                                    border: UiRect::all(Val::Px(5.)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                border_color: BorderColor(Color::BLACK),
                                background_color: Color::WHITE.into(),
                                ..default()
                            })
                            .with_children(|parent| {
                                parent.spawn(TextBundle::from_section(
                                    "Restart",
                                    TextStyle {
                                        font_size: 40.,
                                        color: Color::BLACK,
                                        ..default()
                                    },
                                ));
                            });
                    });
            }
        }
    }
}

fn restart_button(
    mut interaction_query: Query<&Interaction, (Changed<Interaction>, With<Button>)>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for interaction in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => *next_state = NextState(Some(AppState::Restarting)),
            _ => {}
        }
    }
}

fn restart(mut next_state: ResMut<NextState<AppState>>) {
    *next_state = NextState(Some(AppState::Running));
}

fn teardown(
    mut commands: Commands,
    entities: Query<Entity, Without<bevy::window::PrimaryWindow>>,
    mut score: ResMut<Score>,
) {
    for entity in entities.iter() {
        commands.entity(entity).despawn();
        score.0 = 0;
    }
}

fn limit_player_bounds(mut query: Query<&mut Transform, With<Player>>) {
    for mut transform in query.iter_mut() {
        if transform.translation.x > SCREEN_DIMENSIONS.x / 2. - PLAYER_DIMENSIONS.x / 2. {
            transform.translation.x = SCREEN_DIMENSIONS.x / 2. - PLAYER_DIMENSIONS.x / 2.;
        } else if transform.translation.x < -SCREEN_DIMENSIONS.x / 2. + PLAYER_DIMENSIONS.x / 2. {
            transform.translation.x = -SCREEN_DIMENSIONS.x / 2. + PLAYER_DIMENSIONS.x / 2.;
        }

        if transform.translation.y > SCREEN_DIMENSIONS.y / 2. - PLAYER_DIMENSIONS.y / 2. {
            transform.translation.y = SCREEN_DIMENSIONS.y / 2. - PLAYER_DIMENSIONS.y / 2.;
        } else if transform.translation.y < -SCREEN_DIMENSIONS.y / 2. + PLAYER_DIMENSIONS.y / 2. {
            transform.translation.y = -SCREEN_DIMENSIONS.y / 2. + PLAYER_DIMENSIONS.y / 2.;
        }
    }
}
