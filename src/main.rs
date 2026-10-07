mod locomotion;

use bevy::prelude::*;

use locomotion::components::MoveIntent;
use locomotion::sensors::Obstacle;
use locomotion::state::{Locomotion, LocomotionState};
use locomotion::tuning::LocomotionTuning;
use locomotion::{LocomotionPlugin, LocomotionSet};

#[derive(Component)]
struct Player;

/// Visual capsule, a child of the player so it can squash per stance.
#[derive(Component)]
struct PlayerBody;

#[derive(Component)]
struct Hud;

const CAMERA_OFFSET: Vec3 = Vec3::new(0.0, 4.5, 7.0);

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, LocomotionPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, read_player_input.in_set(LocomotionSet::Input))
        .add_systems(
            Update,
            (follow_camera, update_body_visual, update_hud).after(LocomotionSet::Integrate),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(60.0, 60.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.35, 0.4, 0.35))),
    ));

    // (center, size): a vaultable low wall, a too-tall wall, a too-deep crate.
    let obstacles = [
        (Vec3::new(0.0, 0.45, -5.0), Vec3::new(4.0, 0.9, 0.3)),
        (Vec3::new(6.0, 1.25, -5.0), Vec3::new(4.0, 2.5, 0.3)),
        (Vec3::new(-6.0, 0.5, -5.0), Vec3::new(2.0, 1.0, 2.0)),
    ];
    let wall_material = materials.add(Color::srgb(0.55, 0.5, 0.45));
    for (center, size) in obstacles {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(wall_material.clone()),
            Transform::from_translation(center),
            Obstacle { half_extents: size * 0.5 },
        ));
    }

    let tuning = LocomotionTuning::default();
    let body_height = tuning.standing_height - 2.0 * tuning.radius;
    commands
        .spawn((Player, Locomotion::default(), tuning.clone(), Visibility::default()))
        .with_child((
            PlayerBody,
            Mesh3d(meshes.add(Capsule3d::new(tuning.radius, body_height))),
            MeshMaterial3d(materials.add(Color::srgb(0.8, 0.45, 0.2))),
            Transform::from_xyz(0.0, tuning.standing_height * 0.5, 0.0),
        ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(CAMERA_OFFSET).looking_at(Vec3::Y, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight { shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(4.0, 10.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Hud,
        Text::new(""),
        Node { position_type: PositionType::Absolute, top: Val::Px(12.0), left: Val::Px(12.0), ..default() },
    ));
}

/// WASD/left stick move, Shift/L3 sprint, Alt walks, Space/A jump-vault,
/// C/B crouch, Z/Y prone.
fn read_player_input(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    camera: Query<&Transform, With<Camera3d>>,
    mut players: Query<&mut MoveIntent, With<Player>>,
) {
    let Ok(cam) = camera.single() else { return };

    let mut stick = Vec2::ZERO;
    let key_axis = |neg, pos| keys.pressed(pos) as i8 as f32 - keys.pressed(neg) as i8 as f32;
    stick.x = key_axis(KeyCode::KeyA, KeyCode::KeyD);
    stick.y = key_axis(KeyCode::KeyS, KeyCode::KeyW);
    stick = stick.normalize_or_zero();
    if keys.pressed(KeyCode::AltLeft) {
        stick *= 0.5;
    }

    let mut sprint = keys.pressed(KeyCode::ShiftLeft);
    let mut jump = keys.just_pressed(KeyCode::Space);
    let mut crouch = keys.just_pressed(KeyCode::KeyC);
    let mut prone = keys.just_pressed(KeyCode::KeyZ);
    for pad in &gamepads {
        let left = pad.left_stick();
        if left.length() > stick.length() {
            stick = left.clamp_length_max(1.0);
        }
        sprint |= pad.pressed(GamepadButton::LeftThumb);
        jump |= pad.just_pressed(GamepadButton::South);
        crouch |= pad.just_pressed(GamepadButton::East);
        prone |= pad.just_pressed(GamepadButton::North);
    }

    // Camera-relative, flattened onto the ground plane.
    let forward = cam.forward().as_vec3().with_y(0.0).normalize_or_zero();
    let right = cam.right().as_vec3().with_y(0.0).normalize_or_zero();
    let direction = right * stick.x + forward * stick.y;

    for mut intent in &mut players {
        *intent = MoveIntent { direction, sprint, jump, crouch, prone, ..default() };
    }
}

fn follow_camera(
    time: Res<Time>,
    player: Query<&Transform, (With<Player>, Without<Camera3d>)>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
) {
    let (Ok(player), Ok(mut cam)) = (player.single(), camera.single_mut()) else { return };
    let target = player.translation + CAMERA_OFFSET;
    let t = 1.0 - (-6.0 * time.delta_secs()).exp();
    cam.translation = cam.translation.lerp(target, t);
    cam.look_at(player.translation + Vec3::Y, Vec3::Y);
}

/// Squash the capsule toward the current stance height (stand-in for animation).
fn update_body_visual(
    time: Res<Time>,
    player: Query<(&Locomotion, &LocomotionTuning), With<Player>>,
    mut body: Query<&mut Transform, With<PlayerBody>>,
) {
    let (Ok((loco, tuning)), Ok(mut transform)) = (player.single(), body.single_mut()) else { return };
    let target = tuning.height(loco.state.stance()) / tuning.standing_height;
    let t = 1.0 - (-12.0 * time.delta_secs()).exp();
    transform.scale.y += (target - transform.scale.y) * t;
    transform.translation.y = tuning.standing_height * 0.5 * transform.scale.y;
}

fn update_hud(
    player: Query<&Locomotion, With<Player>>,
    mut hud: Query<&mut Text, With<Hud>>,
) {
    let (Ok(loco), Ok(mut text)) = (player.single(), hud.single_mut()) else { return };
    let name = match loco.state {
        LocomotionState::Traversal(_) => "Traversal".to_string(),
        other => format!("{other:?}"),
    };
    text.0 = format!(
        "{name}  ({:.2}s)\nWASD move | Shift sprint | Alt walk | Space jump/vault | C crouch | Z prone",
        loco.time_in_state
    );
}
