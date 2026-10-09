mod camera;
mod locomotion;
mod rig;
mod weapon;

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use bevy_rapier3d::prelude::{Collider, ColliderMassProperties, Friction, Restitution, RigidBody};

use camera::{CameraRigPlugin, OrbitCamera};

use locomotion::components::MoveIntent;
use locomotion::sensors::Obstacle;
use locomotion::sprint::SprintStamina;
use locomotion::state::{Locomotion, LocomotionState};
use locomotion::tuning::LocomotionTuning;
use locomotion::{LocomotionPlugin, LocomotionSet};
use rig::RigPlugin;
use weapon::damage::{Target, TargetKind};
use weapon::melee::{MeleeState, MeleeTuning, SwingPhase};
use weapon::systems::{HitFlash, ShotEffects};
use weapon::{AmmoSettings, Arsenal, Loadout, Silencer, WeaponId, WeaponInput, WeaponPlugin};

#[derive(Component)]
struct Player;

/// Visual humanoid, a child of the player so it can squash per stance.
#[derive(Component)]
struct PlayerBody;

/// Stand-in weapon model: a stick in the right hand, one per weapon.
#[derive(Component)]
struct HeldStick(WeaponId);

/// Stick (length, thickness) in metres for each weapon that has one.
fn stick_size(weapon: WeaponId) -> Option<(f32, f32)> {
    match weapon {
        WeaponId::Unarmed => None,
        WeaponId::HuntingRifle => Some((1.1, 0.06)),
        WeaponId::Pistol => Some((0.35, 0.05)),
        WeaponId::Melee => Some((0.25, 0.04)),
    }
}

#[derive(Component)]
struct Hud;

/// Centre dot shown while aiming over the shoulder.
#[derive(Component)]
struct Crosshair;

/// Full-screen cross lines shown while looking through the rifle scope.
#[derive(Component)]
struct ScopeOverlay;

/// Mirrors the game's accessibility options (`sprinting-toggle`).
#[derive(Resource)]
struct ControlSettings {
    sprint_toggle: bool,
    /// Holding crouch this long goes prone instead of toggling crouch.
    crouch_hold_time: f32,
    /// Radians per pixel of mouse motion.
    mouse_sensitivity: f32,
    /// Radians per second at full right-stick deflection.
    stick_look_speed: f32,
}

impl Default for ControlSettings {
    fn default() -> Self {
        Self { sprint_toggle: false, crouch_hold_time: 0.3, mouse_sensitivity: 0.0025, stick_look_speed: 3.0 }
    }
}

#[derive(Default)]
struct InputLatch {
    sprint_on: bool,
    crouch_held: Option<f32>,
    prone_fired: bool,
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, LocomotionPlugin, CameraRigPlugin, WeaponPlugin, RigPlugin))
        .init_resource::<ControlSettings>()
        .add_systems(Startup, setup)
        .add_systems(Update, (grab_cursor, read_player_input).chain().in_set(LocomotionSet::Input))
        .add_systems(
            Update,
            (update_body_visual, update_held_stick, update_aim_ui, update_hud).after(LocomotionSet::Integrate),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(500.0, 500.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.35, 0.4, 0.35))),
    ));
    // The visual plane is at y = 0; the collider's top face matches it.
    commands.spawn((Collider::cuboid(250.0, 0.5, 250.0), Transform::from_xyz(0.0, -0.5, 0.0)));

    // (min corner, max corner) of each box.
    let course = [
        // Ahead (-Z): vault wall, too-tall wall, curb you can step onto.
        (Vec3::new(-2.0, 0.0, -5.15), Vec3::new(2.0, 0.9, -4.85)),
        (Vec3::new(3.0, 0.0, -5.15), Vec3::new(6.0, 2.5, -4.85)),
        (Vec3::new(-6.0, 0.0, -3.0), Vec3::new(-3.0, 0.2, -2.0)),
        // Left (-X): staircase of platforms, 0.5 / 1.5 / 3.0 / 4.5 m. Climb up, jump off.
        (Vec3::new(-9.0, 0.0, -10.0), Vec3::new(-6.0, 0.5, -7.0)),
        (Vec3::new(-12.0, 0.0, -10.0), Vec3::new(-9.0, 1.5, -7.0)),
        (Vec3::new(-15.0, 0.0, -10.0), Vec3::new(-12.0, 3.0, -7.0)),
        (Vec3::new(-18.0, 0.0, -10.0), Vec3::new(-15.0, 4.5, -7.0)),
        // Right (+X): 1 m platform to climb or catch with a running jump.
        (Vec3::new(7.0, 0.0, -10.0), Vec3::new(10.0, 1.0, -7.0)),
        // Behind (+Z): crouch tunnel (1.3 m roof) leading into a prone crawlspace (0.7 m).
        (Vec3::new(-1.0, 1.3, 5.0), Vec3::new(1.0, 1.6, 9.0)),
        (Vec3::new(-1.0, 0.7, 9.0), Vec3::new(1.0, 1.6, 12.0)),
        (Vec3::new(-1.3, 0.0, 5.0), Vec3::new(-1.0, 1.6, 12.0)),
        (Vec3::new(1.0, 0.0, 5.0), Vec3::new(1.3, 1.6, 12.0)),
    ];
    let wall_material = materials.add(Color::srgb(0.55, 0.5, 0.45));
    for (min, max) in course {
        let size = max - min;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(wall_material.clone()),
            Transform::from_translation((min + max) * 0.5),
            Obstacle { half_extents: size * 0.5 },
        ));
    }

    // Level dressing: static, original shapes. Each is an `Obstacle`, so it
    // gets a Rapier collider and blocks shots and the camera.
    let container_material = materials.add(Color::srgb(0.7, 0.28, 0.2));
    let concrete_material = materials.add(Color::srgb(0.6, 0.6, 0.58));
    // (centre, half extents, material): shipping containers, a stack and jersey barriers.
    let dressing = [
        (Vec3::new(14.0, 1.3, 4.0), Vec3::new(3.0, 1.3, 1.2), &container_material),
        (Vec3::new(14.0, 3.9, 4.0), Vec3::new(3.0, 1.3, 1.2), &container_material),
        (Vec3::new(14.0, 1.3, -1.0), Vec3::new(3.0, 1.3, 1.2), &container_material),
        (Vec3::new(-8.0, 0.4, 6.0), Vec3::new(1.5, 0.4, 0.3), &concrete_material),
        (Vec3::new(-11.0, 0.4, 6.0), Vec3::new(1.5, 0.4, 0.3), &concrete_material),
        (Vec3::new(-9.5, 0.4, 9.0), Vec3::new(0.3, 0.4, 1.5), &concrete_material),
    ];
    for (center, half, material) in dressing {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(half * 2.0))),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(center),
            Obstacle { half_extents: half },
        ));
    }

    // Pushable props near the start: crates and barrels the player shoves
    // around (the character controller applies impulses to dynamic bodies).
    let crate_material = materials.add(Color::srgb(0.6, 0.45, 0.25));
    let barrel_material = materials.add(Color::srgb(0.25, 0.35, 0.5));
    for (i, (x, z)) in [(2.5, 2.0), (2.5, 3.0), (3.6, 2.5), (-3.0, 3.0), (-4.2, 4.0)].into_iter().enumerate() {
        let (shape, collider, half_y, mass) = if i % 2 == 0 {
            (Mesh::from(Cuboid::from_length(0.8)), Collider::cuboid(0.4, 0.4, 0.4), 0.4, 30.0)
        } else {
            (Mesh::from(Cylinder::new(0.3, 0.9)), Collider::cylinder(0.45, 0.3), 0.45, 20.0)
        };
        commands.spawn((
            RigidBody::Dynamic,
            collider,
            ColliderMassProperties::Mass(mass),
            Friction::coefficient(0.8),
            Restitution::coefficient(0.05),
            Mesh3d(meshes.add(shape)),
            MeshMaterial3d(if i % 2 == 0 { crate_material.clone() } else { barrel_material.clone() }),
            Transform::from_xyz(x, half_y + 0.01, z),
        ));
    }

    // Shooting range down -Z, past the course. Distances span the pistol's
    // 30 m falloff and the start of the rifle's 150-300 m falloff.
    let range = [
        (Vec3::new(0.0, 0.0, -20.0), TargetKind::Human),
        (Vec3::new(-4.0, 0.0, -30.0), TargetKind::Runner),
        (Vec3::new(5.0, 0.0, -40.0), TargetKind::Bloater),
        (Vec3::new(-2.0, 0.0, -70.0), TargetKind::Human),
        (Vec3::new(3.0, 0.0, -160.0), TargetKind::Human),
        (Vec3::new(0.0, 0.0, -240.0), TargetKind::Human),
    ];
    // Melee dummies off to the right of the start.
    let range = range.into_iter().chain([
        (Vec3::new(4.0, 0.0, 1.0), TargetKind::Human),
        (Vec3::new(5.5, 0.0, 3.0), TargetKind::Runner),
        (Vec3::new(4.0, 0.0, 5.0), TargetKind::Bloater),
    ]);
    for (feet, kind) in range {
        let (half, color) = match kind {
            TargetKind::Human => (Vec3::new(0.3, 0.9, 0.3), Color::srgb(0.35, 0.45, 0.6)),
            TargetKind::Runner => (Vec3::new(0.3, 0.85, 0.3), Color::srgb(0.6, 0.65, 0.25)),
            TargetKind::Bloater => (Vec3::new(0.55, 1.05, 0.55), Color::srgb(0.55, 0.25, 0.2)),
        };
        commands.spawn((
            Target::new(kind, half),
            HitFlash::new(color),
            Mesh3d(meshes.add(Capsule3d::new(half.x, 2.0 * (half.y - half.x)))),
            MeshMaterial3d(materials.add(color)),
            Transform::from_translation(feet + Vec3::Y * half.y),
        ));
    }

    let tuning = LocomotionTuning::default();
    // Jointed placeholder humanoid (see rig.rs). The rig root carries PlayerBody.
    let player = commands
        .spawn((Player, Locomotion::default(), Loadout::default(), tuning.clone(), Visibility::default()))
        .id();
    rig::spawn_rig(&mut commands, &mut meshes, &mut materials, player, PlayerBody);

    let wood = materials.add(Color::srgb(0.35, 0.22, 0.12));
    let steel = materials.add(Color::srgb(0.15, 0.15, 0.17));
    let rust = materials.add(Color::srgb(0.45, 0.3, 0.22));
    for weapon in [WeaponId::HuntingRifle, WeaponId::Pistol, WeaponId::Melee] {
        // (size, offset from the weapon centre, material). -Z is the muzzle end.
        let parts: Vec<(Vec3, Vec3, &Handle<StandardMaterial>)> = match weapon {
            WeaponId::HuntingRifle => vec![
                (Vec3::new(0.03, 0.03, 0.55), Vec3::new(0.0, 0.0, -0.28), &steel),
                (Vec3::new(0.05, 0.08, 0.30), Vec3::new(0.0, 0.0, 0.02), &steel),
                (Vec3::new(0.05, 0.11, 0.30), Vec3::new(0.0, -0.03, 0.40), &wood),
                (Vec3::new(0.04, 0.04, 0.22), Vec3::new(0.0, 0.07, -0.02), &steel),
            ],
            WeaponId::Pistol => vec![
                (Vec3::new(0.035, 0.04, 0.20), Vec3::new(0.0, 0.0, -0.07), &steel),
                (Vec3::new(0.035, 0.10, 0.045), Vec3::new(0.0, -0.07, 0.08), &wood),
            ],
            _ => vec![
                (Vec3::new(0.04, 0.04, 0.20), Vec3::new(0.0, 0.0, 0.03), &wood),
                (Vec3::new(0.09, 0.06, 0.10), Vec3::new(0.0, 0.0, -0.10), &rust),
            ],
        };
        commands
            .spawn((HeldStick(weapon), Transform::default(), Visibility::Hidden, ChildOf(player)))
            .with_children(|stick| {
                for (size, offset, material) in parts {
                    stick.spawn((
                        Mesh3d(meshes.add(Cuboid::from_size(size))),
                        MeshMaterial3d(material.clone()),
                        Transform::from_translation(offset),
                    ));
                }
            });
    }

    commands.spawn(OrbitCamera::new(player));
    commands.spawn((
        DirectionalLight { shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(4.0, 10.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Hud,
        Text::new(""),
        Node { position_type: PositionType::Absolute, top: Val::Px(12.0), left: Val::Px(12.0), ..default() },
    ));

    commands.spawn((
        Crosshair,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(50.0),
            width: Val::Px(4.0),
            height: Val::Px(4.0),
            margin: UiRect { left: Val::Px(-2.0), top: Val::Px(-2.0), ..default() },
            ..default()
        },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
        Visibility::Hidden,
    ));
    let line = |horizontal: bool| {
        let (width, height, left, top) = if horizontal {
            (Val::Percent(100.0), Val::Px(1.0), Val::Px(0.0), Val::Percent(50.0))
        } else {
            (Val::Px(1.0), Val::Percent(100.0), Val::Percent(50.0), Val::Px(0.0))
        };
        (Node { position_type: PositionType::Absolute, left, top, width, height, ..default() }, BackgroundColor(Color::BLACK))
    };
    commands.spawn((
        ScopeOverlay,
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        Visibility::Hidden,
        children![line(true), line(false)],
    ));
}

/// Click locks the mouse to the window for camera look; Esc releases it.
fn grab_cursor(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut cursors: Query<&mut CursorOptions>,
) {
    for mut cursor in &mut cursors {
        if mouse.just_pressed(MouseButton::Left) {
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
        } else if keys.just_pressed(KeyCode::Escape) {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        }
    }
}

/// WASD/left stick move, mouse/right stick look, Shift/L1 sprint, Alt walks,
/// Space/Cross jump-traverse, C/Circle tap crouch or hold prone, Z prone,
/// right mouse/L2 aim, left mouse/R2 fire, R reload, Shift/L1 holds breath
/// while scoped, F or left mouse (not aiming)/Square melee (Square reloads
/// while aiming), Q/R1 dodge, V/D-pad down toggles the silencer, 1-4 or D-pad
/// pick a weapon, F1 toggles sprint mode, F2 infinite ammo.
#[allow(clippy::too_many_arguments)]
fn read_player_input(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    gamepads: Query<&Gamepad>,
    cursors: Query<&CursorOptions>,
    mut cameras: Query<&mut OrbitCamera>,
    mut settings: ResMut<ControlSettings>,
    mut ammo_settings: ResMut<AmmoSettings>,
    mut latch: Local<InputLatch>,
    mut players: Query<(&mut MoveIntent, &mut Loadout, &mut WeaponInput, &MeleeState), With<Player>>,
) {
    let Ok(mut cam) = cameras.single_mut() else { return };
    if keys.just_pressed(KeyCode::F1) {
        settings.sprint_toggle = !settings.sprint_toggle;
        latch.sprint_on = false;
    }
    if keys.just_pressed(KeyCode::F2) {
        ammo_settings.infinite_reserve = !ammo_settings.infinite_reserve;
    }

    let key_axis = |neg, pos| keys.pressed(pos) as i8 as f32 - keys.pressed(neg) as i8 as f32;
    let mut stick =
        Vec2::new(key_axis(KeyCode::KeyA, KeyCode::KeyD), key_axis(KeyCode::KeyS, KeyCode::KeyW))
            .normalize_or_zero();
    if keys.pressed(KeyCode::AltLeft) {
        stick *= 0.5;
    }

    let mut sprint_held = keys.pressed(KeyCode::ShiftLeft);
    let mut sprint_pressed = keys.just_pressed(KeyCode::ShiftLeft);
    let mut jump = keys.just_pressed(KeyCode::Space);
    let mut crouch_held = keys.pressed(KeyCode::KeyC);
    let mut prone = keys.just_pressed(KeyCode::KeyZ);
    let mut aim_held = mouse.pressed(MouseButton::Right);
    let mut fire = mouse.just_pressed(MouseButton::Left);
    let mut reload = keys.just_pressed(KeyCode::KeyR);
    let mut toggle_silencer = keys.just_pressed(KeyCode::KeyV);
    let mut melee = keys.just_pressed(KeyCode::KeyF);
    let mut square = false;
    let mut evade = keys.just_pressed(KeyCode::KeyQ);
    let mut pick = [
        (KeyCode::Digit1, WeaponId::Unarmed),
        (KeyCode::Digit2, WeaponId::HuntingRifle),
        (KeyCode::Digit3, WeaponId::Pistol),
        (KeyCode::Digit4, WeaponId::Melee),
    ]
    .into_iter()
    .find_map(|(key, weapon)| keys.just_pressed(key).then_some(weapon));

    let mut holster_cycle = false;
    let mouse_locked = cursors.iter().any(|c| c.grab_mode != CursorGrabMode::None);
    let mut look = if mouse_locked { -mouse_motion.delta * settings.mouse_sensitivity } else { Vec2::ZERO };

    for pad in &gamepads {
        let left = pad.left_stick();
        if left.length() > stick.length() {
            stick = left.clamp_length_max(1.0);
        }
        look -= Vec2::new(pad.right_stick().x, -pad.right_stick().y)
            * settings.stick_look_speed
            * time.delta_secs();
        sprint_held |= pad.pressed(GamepadButton::LeftTrigger);
        sprint_pressed |= pad.just_pressed(GamepadButton::LeftTrigger);
        jump |= pad.just_pressed(GamepadButton::South);
        crouch_held |= pad.pressed(GamepadButton::East);
        aim_held |= pad.pressed(GamepadButton::LeftTrigger2);
        fire |= pad.just_pressed(GamepadButton::RightTrigger2);
        square |= pad.just_pressed(GamepadButton::West);
        evade |= pad.just_pressed(GamepadButton::RightTrigger);
        toggle_silencer |= pad.just_pressed(GamepadButton::DPadDown);
        if pad.just_pressed(GamepadButton::DPadUp) {
            holster_cycle = true;
        } else if pad.just_pressed(GamepadButton::DPadRight) {
            pick = Some(WeaponId::HuntingRifle);
        } else if pad.just_pressed(GamepadButton::DPadLeft) {
            pick = Some(WeaponId::Pistol);
        }
    }
    cam.look += look;

    let Ok((mut intent, mut loadout, mut weapon_input, melee_state)) = players.single_mut() else { return };
    // D-pad up swaps between bare hands and the melee weapon.
    if holster_cycle {
        pick = Some(if loadout.equipped == WeaponId::Melee { WeaponId::Unarmed } else { WeaponId::Melee });
    }
    if let Some(weapon) = pick {
        loadout.equipped = weapon;
    }
    loadout.aiming = aim_held && loadout.equipped.can_aim();
    if loadout.aiming {
        latch.sprint_on = false;
    }
    // Square is melee, or reload while aiming. The fire button swings when
    // not aiming. The sprint button doubles as hold-breath, since aiming can't sprint.
    if loadout.aiming {
        reload |= square;
    } else {
        melee |= square || fire;
    }
    *weapon_input = WeaponInput { fire, reload, hold_breath: sprint_held, toggle_silencer, melee };

    // Toggle sprint stays latched until the stick is released.
    let sprint = !loadout.aiming && if settings.sprint_toggle {
        if sprint_pressed {
            latch.sprint_on = !latch.sprint_on;
        }
        if stick.length() < 0.15 {
            latch.sprint_on = false;
        }
        latch.sprint_on
    } else {
        sprint_held
    };

    // Crouch button: tap toggles crouch, hold goes prone.
    let mut crouch = false;
    match (crouch_held, latch.crouch_held) {
        (true, None) => latch.crouch_held = Some(0.0),
        (true, Some(held)) => {
            let held = held + time.delta_secs();
            latch.crouch_held = Some(held);
            if held >= settings.crouch_hold_time && !latch.prone_fired {
                latch.prone_fired = true;
                prone = true;
            }
        }
        (false, Some(_)) => {
            crouch = !latch.prone_fired;
            latch.crouch_held = None;
            latch.prone_fired = false;
        }
        (false, None) => {}
    }

    // Camera-relative on the ground plane.
    let (forward, right) = cam.ground_axes();
    let mut direction = right * stick.x + forward * stick.y;
    let mut strafe_facing = loadout.aiming.then_some(forward);
    // Dodge along the stick, or straight back with no stick.
    let evade = evade.then(|| if stick.length() > 0.15 { direction } else { -forward });
    // Plant the feet and face the swing while attacking.
    if let Some(swing) = melee_state.swing {
        direction = Vec3::ZERO;
        strafe_facing = Some(swing.direction);
    }

    *intent = MoveIntent { direction, sprint, jump, crouch, prone, strafe_facing, evade };
}

/// Hides the body while looking through the scope (the camera sits inside the head).
/// Crouch and prone are posed by the rig, not by squashing it.
fn update_body_visual(
    player: Query<&Loadout, With<Player>>,
    mut body: Query<&mut Visibility, With<PlayerBody>>,
) {
    let (Ok(loadout), Ok(mut visibility)) = (player.single(), body.single_mut()) else { return };
    *visibility = if scoped(loadout) { Visibility::Hidden } else { Visibility::Inherited };
}

/// Shows the equipped weapon's stick in the right hand: lowered while
/// exploring, raised to the shoulder along the camera pitch while aiming.
fn update_held_stick(
    time: Res<Time>,
    player: Query<(&Locomotion, &LocomotionTuning, &Loadout, &MeleeState), With<Player>>,
    camera: Query<&OrbitCamera>,
    mut sticks: Query<(&HeldStick, &mut Transform, &mut Visibility)>,
) {
    let (Ok((loco, tuning, loadout, melee)), Ok(cam)) = (player.single(), camera.single()) else { return };
    let height = tuning.height(loco.state.stance());
    let t = 1.0 - (-15.0 * time.delta_secs()).exp();
    for (stick, mut transform, mut visibility) in &mut sticks {
        // The scope camera sits where the rifle would be.
        let shown = stick.0 == loadout.equipped && !scoped(loadout);
        *visibility = if shown { Visibility::Inherited } else { Visibility::Hidden };
        if !shown {
            continue;
        }
        let (length, _) = stick_size(stick.0).expect("has a stick");
        // Player-local space: -Z forward, +X right.
        let (grip, pitch) = if loadout.aiming {
            (Vec3::new(0.2, height * 0.78, -0.25), cam.pitch)
        } else {
            let lowered = match stick.0 {
                WeaponId::HuntingRifle => -0.6,
                WeaponId::Pistol => -1.2,
                _ => -1.4,
            };
            (Vec3::new(0.28, height * 0.5, -0.1), lowered)
        };
        let mut rotation = Quat::from_rotation_x(pitch);
        // Swing: cocked back over the right shoulder, then across to the left.
        // Alternate combo swings go the other way; the finisher comes down overhead.
        let mut grip = grip;
        if let Some(swing) = melee.swing {
            let t = MeleeTuning::for_weapon(loadout.equipped);
            let side = if swing.combo % 2 == 0 { 1.0 } else { -1.0 };
            let (yaw, swing_pitch) = match swing.phase {
                SwingPhase::Windup => (1.2 * side, 0.6),
                SwingPhase::Active => {
                    let k = swing.time / t.active;
                    (side * (1.2 - 2.4 * k), 0.6 - 0.9 * k)
                }
                SwingPhase::Recovery => (-1.2 * side, -0.3),
            };
            let (yaw, swing_pitch) = if swing.is_finisher(&t) { (0.0, swing_pitch * 2.0) } else { (yaw, swing_pitch) };
            rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(swing_pitch);
            grip = Vec3::new(0.15, height * 0.7, -0.2);
        }
        // The mesh is centred, so push it forward half its length from the grip.
        let center = grip + rotation * Vec3::new(0.0, 0.0, -0.5 * length);
        transform.translation = transform.translation.lerp(center, t);
        transform.rotation = transform.rotation.slerp(rotation, t);
    }
}

fn scoped(loadout: &Loadout) -> bool {
    loadout.aiming && loadout.equipped == WeaponId::HuntingRifle
}

fn update_aim_ui(
    player: Query<&Loadout, With<Player>>,
    mut crosshair: Query<&mut Visibility, (With<Crosshair>, Without<ScopeOverlay>)>,
    mut scope: Query<&mut Visibility, (With<ScopeOverlay>, Without<Crosshair>)>,
) {
    let Ok(loadout) = player.single() else { return };
    let show = |on: bool| if on { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut crosshair {
        *v = show(loadout.aiming && !scoped(loadout));
    }
    for mut v in &mut scope {
        *v = show(scoped(loadout));
    }
}

fn update_hud(
    settings: Res<ControlSettings>,
    effects: Res<ShotEffects>,
    ammo_settings: Res<AmmoSettings>,
    player: Query<(&Locomotion, &SprintStamina, &Loadout, &Arsenal, Option<&Silencer>, &MeleeState), With<Player>>,
    mut hud: Query<&mut Text, With<Hud>>,
) {
    let (Ok((loco, stamina, loadout, arsenal, silencer, melee)), Ok(mut text)) = (player.single(), hud.single_mut())
    else {
        return;
    };
    let state = match loco.state {
        LocomotionState::Traversal(m) => format!("Traversal {:?} {:?} {:?}", m.kind, m.bucket, m.entry),
        LocomotionState::Fall { .. } => "Fall".to_string(),
        other => format!("{other:?}"),
    };
    let sprint_mode = if settings.sprint_toggle { "toggle" } else { "hold" };
    let mut weapon = format!("Weapon {:?}", loadout.equipped);
    if let Some(ammo) = arsenal.ammo(loadout.equipped) {
        if ammo_settings.infinite_reserve {
            weapon += &format!("  {}/inf", ammo.loaded);
        } else {
            weapon += &format!("  {}/{}", ammo.loaded, ammo.reserve);
        }
    }
    if let Some(s) = silencer.filter(|_| loadout.equipped == WeaponId::Pistol) {
        weapon += &format!("  silencer {}/{}", s.health, Silencer::MAX_HEALTH);
    }
    if let Some(swing) = melee.swing {
        weapon += &format!("  swing {} {:?}", swing.combo + 1, swing.phase);
    }
    if loadout.aiming {
        weapon += "  (aiming)";
    }
    if arsenal.reload.is_some() {
        weapon += "  reloading";
    } else if arsenal.cooldown > 0.0 && loadout.equipped == WeaponId::HuntingRifle {
        weapon += "  working bolt";
    }
    if arsenal.breath.holding {
        weapon += "  holding breath";
    } else if arsenal.breath.recovering > 0.0 {
        weapon += "  out of breath";
    }
    let hit = match &effects.last_hit {
        Some(h) => format!(
            "Last hit: {:?}{} {:.0} dmg at {:.1} m -> {:.0} hp{}",
            h.kind,
            if h.headshot { " (head)" } else { "" },
            h.damage,
            h.distance,
            h.health_left,
            if h.health_left <= 0.0 { ", down" } else { "" },
        ),
        None => String::new(),
    };
    text.0 = format!(
        "{state}  ({:.2}s)\nStamina {:.0}{}  | sprint: {sprint_mode} (F1)\n{weapon}\n{hit}\n\n\
         WASD move | Shift sprint | Alt walk | Space jump/vault/climb | C tap crouch, hold prone | Z prone\n\
         Click to lock mouse, Esc to release | RMB aim | LMB fire | R reload | Shift hold breath (scoped) \
         | V silencer | F melee (LMB when not aiming) | Q dodge | 1 unarmed, 2 rifle, 3 pistol, 4 melee | F2 infinite ammo",
        loco.time_in_state,
        stamina.value,
        if stamina.exhausted { " (exhausted)" } else { "" },
    );
}
