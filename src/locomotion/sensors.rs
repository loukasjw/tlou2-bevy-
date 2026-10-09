//! World queries and collision. Rapier owns the static world (every `Obstacle`
//! gets a cuboid collider) and the kinematic capsule controller that moves
//! characters; this module turns its ray casts into the sensor components the
//! state machine reads. Only the edge-traversal probe still works from the
//! `Obstacle` boxes, because it needs the exact top/exit geometry of a ledge.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use super::components::{EnvironmentSensors, GroundSensor, KinematicBody, MoveIntent};
use super::state::{Locomotion, LocomotionState};
use super::traversal::TraversalCandidate;
use super::tuning::LocomotionTuning;

const FLOOR_Y: f32 = 0.0;
const GROUND_SKIN: f32 = 0.06;
const LANDING_MARGIN: f32 = 0.15;
const ONTO_MARGIN: f32 = 0.1;

#[derive(Component, Debug, Clone, Copy)]
pub struct Obstacle {
    pub half_extents: Vec3,
}

#[derive(Clone, Copy)]
struct Aabb {
    min: Vec3,
    max: Vec3,
}

impl Aabb {
    fn footprint_contains(&self, p: Vec2, margin: f32) -> bool {
        p.x >= self.min.x - margin
            && p.x <= self.max.x + margin
            && p.y >= self.min.z - margin
            && p.y <= self.max.z + margin
    }
}

fn collect(obstacles: &Query<(&Transform, &Obstacle)>) -> Vec<Aabb> {
    obstacles
        .iter()
        .map(|(t, o)| Aabb { min: t.translation - o.half_extents, max: t.translation + o.half_extents })
        .collect()
}

/// Highest walkable surface under `p` (box data, used by the traversal probe) that is no more than a step above `feet_y`.
fn box_ground_height_at(boxes: &[Aabb], p: Vec2, feet_y: f32, t: &LocomotionTuning) -> f32 {
    boxes
        .iter()
        .filter(|b| b.footprint_contains(p, 0.5 * t.radius) && b.max.y <= feet_y + t.step_height)
        .map(|b| b.max.y)
        .fold(FLOOR_Y, f32::max)
}

/// Offsets of the probe rays under/over the capsule footprint.
fn probe_offsets(radius: f32) -> [Vec3; 5] {
    let r = radius * 0.7;
    [Vec3::ZERO, Vec3::X * r, Vec3::NEG_X * r, Vec3::Z * r, Vec3::NEG_Z * r]
}

fn cast(ctx: &RapierContext, origin: Vec3, dir: Vec3, max: f32) -> Option<f32> {
    // A ray that starts inside a collider reports distance 0; ignore it so
    // probes buried in a wall don't read as ground or ceiling.
    ctx.cast_ray(origin, dir, max, true, QueryFilter::default()).map(|(_, toi)| toi).filter(|&toi| toi > 1e-4)
}

/// Highest surface under the footprint that is no more than a step above `feet`.
fn ground_height_at(ctx: &RapierContext, feet: Vec3, t: &LocomotionTuning) -> f32 {
    let from = feet + Vec3::Y * t.step_height;
    probe_offsets(t.radius)
        .iter()
        .filter_map(|o| cast(ctx, from + *o, Vec3::NEG_Y, t.step_height + 200.0))
        .map(|toi| from.y - toi)
        .fold(FLOOR_Y, f32::max)
}

/// Clearance from `feet` up to the lowest overhead surface (above step height).
fn ceiling_at(ctx: &RapierContext, feet: Vec3, t: &LocomotionTuning) -> f32 {
    let from = feet + Vec3::Y * t.step_height;
    probe_offsets(t.radius)
        .iter()
        .filter_map(|o| cast(ctx, from + *o, Vec3::Y, 20.0))
        .map(|toi| t.step_height + toi)
        .fold(f32::INFINITY, f32::min)
}

/// Gives every character a Rapier kinematic controller; its shape follows the
/// stance in `systems::integrate`.
///
/// Rapier only links an entity to its physics context when it has a collider,
/// and characters use a controller-only shape, so the link is added by hand.
pub fn init_controllers(
    mut commands: Commands,
    context: Query<Entity, With<DefaultRapierContext>>,
    new: Query<Entity, Added<Locomotion>>,
) {
    let Ok(context) = context.single() else { return };
    for entity in &new {
        commands.entity(entity).insert((RapierContextEntityLink(context), KinematicCharacterController {
            offset: CharacterLength::Absolute(0.01),
            slide: true,
            // Rapier's autostep doesn't trigger for a round capsule against a
            // low ledge, so stepping is done from the sensed ground height
            // (see `systems::integrate`) and the capsule starts above it.
            autostep: None,
            snap_to_ground: None,
            // Shoving props: roughly a 75 kg person.
            custom_mass: Some(75.0),
            apply_impulse_to_dynamic_bodies: true,
            ..default()
        }));
    }
}

/// Every `Obstacle` becomes a static cuboid in the Rapier world.
pub fn add_obstacle_colliders(mut commands: Commands, new: Query<(Entity, &Obstacle), Added<Obstacle>>) {
    for (entity, o) in &new {
        let h = o.half_extents;
        commands.entity(entity).insert(Collider::cuboid(h.x, h.y, h.z));
    }
}

/// Last frame's controller result: drop velocity into whatever blocked us.
pub fn absorb_blocked_motion(
    bodies: Query<&RigidBody>,
    mut characters: Query<(&Locomotion, &KinematicCharacterControllerOutput, &mut KinematicBody)>,
) {
    for (loco, out, mut body) in &mut characters {
        if matches!(loco.state, LocomotionState::Traversal(_)) {
            continue;
        }
        // Only static geometry stops us dead; shoved props give way.
        let blocked_by_static = out.collisions.iter().any(|c| !matches!(bodies.get(c.entity), Ok(RigidBody::Dynamic)));
        if !blocked_by_static {
            continue;
        }
        let lost = out.desired_translation - out.effective_translation;
        let lost_h = Vec3::new(lost.x, 0.0, lost.z);
        if lost_h.length() > 1e-4 {
            let n = lost_h.normalize();
            let into = body.velocity.dot(n);
            if into > 0.0 {
                body.velocity -= n * into;
            }
        }
        if out.desired_translation.y > 0.0 && lost.y > 1e-4 {
            body.velocity.y = body.velocity.y.min(0.0);
        }
    }
}

pub fn sense_environment(
    rapier: ReadRapierContext,
    obstacles: Query<(&Transform, &Obstacle)>,
    mut characters: Query<
        (
            &Transform,
            &KinematicBody,
            &MoveIntent,
            &LocomotionTuning,
            &mut GroundSensor,
            &mut EnvironmentSensors,
        ),
        Without<Obstacle>,
    >,
) {
    let Ok(ctx) = rapier.single() else { return };
    let boxes = collect(&obstacles);
    for (transform, body, intent, t, mut ground, mut env) in &mut characters {
        let pos = transform.translation;
        ground.height = ground_height_at(&ctx, pos, t);
        ground.grounded = pos.y <= ground.height + GROUND_SKIN && body.velocity.y <= 0.0;

        let dir = intent.direction.xz().normalize_or_zero();
        let moving = intent.direction.length() >= t.move_deadzone;
        env.ceiling_height = ceiling_at(&ctx, pos, t);
        env.ceiling_ahead = if moving {
            ceiling_at(&ctx, pos + Vec3::new(dir.x, 0.0, dir.y) * t.tunnel_probe_distance, t)
        } else {
            f32::INFINITY
        };
        env.traversal = if moving { probe_traversal(&boxes, pos, dir, t) } else { None };
    }
}

/// Nearest edge ahead along the wish direction. Uses intent rather than
/// velocity so you can traverse while pressed flat against a wall.
fn probe_traversal(boxes: &[Aabb], pos: Vec3, dir: Vec2, t: &LocomotionTuning) -> Option<TraversalCandidate> {
    let origin = pos.xz();
    let r = t.radius;
    boxes
        .iter()
        .filter(|b| b.max.y > pos.y + t.step_height && b.min.y <= pos.y + t.step_height)
        .filter_map(|b| {
            // Expand by the capsule radius so we treat the character as a point.
            let min = Vec2::new(b.min.x, b.min.z) - Vec2::splat(r);
            let max = Vec2::new(b.max.x, b.max.z) + Vec2::splat(r);
            let (enter, exit) = ray_aabb_2d(origin, dir, min, max)?;
            if enter > t.traversal_probe_distance {
                return None;
            }
            let top = b.max.y;
            let at = |d: f32, y: f32| {
                let p = origin + dir * d;
                Vec3::new(p.x, y, p.y)
            };
            let over_xz = origin + dir * (exit + LANDING_MARGIN);
            let candidate = TraversalCandidate {
                height: top - pos.y,
                depth: exit - enter - 2.0 * r,
                edge: at(enter + r, top),
                onto: at((enter + 2.0 * r + ONTO_MARGIN).min(exit - r), top),
                over: at(exit + LANDING_MARGIN, box_ground_height_at(boxes, over_xz, pos.y, t)),
            };
            Some((enter, candidate))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, c)| c)
}

/// Slab test. Returns the entry/exit distances along `dir` if the ray hits.
fn ray_aabb_2d(origin: Vec2, dir: Vec2, min: Vec2, max: Vec2) -> Option<(f32, f32)> {
    let inv = dir.recip();
    let t1 = (min - origin) * inv;
    let t2 = (max - origin) * inv;
    let enter = t1.min(t2).max_element();
    let exit = t1.max(t2).min_element();
    (exit >= enter.max(0.0)).then_some((enter.max(0.0), exit))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::time::TimeUpdateStrategy;

    use super::*;
    use crate::locomotion::LocomotionPlugin;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin::default(),
            bevy::mesh::MeshPlugin,
            LocomotionPlugin,
        ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(16)));
        app.world_mut().spawn((Collider::cuboid(100.0, 0.5, 100.0), Transform::from_xyz(0.0, -0.5, 0.0)));
        app
    }

    fn run(app: &mut App, frames: usize) {
        for _ in 0..frames {
            app.update();
        }
    }

    #[test]
    fn character_is_stopped_by_a_wall_and_stays_on_the_floor() {
        let mut app = app();
        let half = Vec3::new(5.0, 3.0, 0.15);
        app.world_mut().spawn((Obstacle { half_extents: half }, Transform::from_xyz(0.0, 3.0, -3.0)));
        let player = app
            .world_mut()
            .spawn((Locomotion::default(), MoveIntent { direction: Vec3::NEG_Z, ..default() }))
            .id();
        run(&mut app, 400);

        let pos = app.world().get::<Transform>(player).unwrap().translation;
        assert!(pos.z > -3.0 + 0.15 + 0.2, "went through the wall: {pos}");
        assert!(pos.z < -2.0, "never reached the wall: {pos}");
        assert!(pos.y.abs() < 0.1, "left the floor: {pos}");
        assert!(app.world().get::<GroundSensor>(player).unwrap().grounded);
    }

    #[test]
    fn character_steps_onto_a_low_curb() {
        let mut app = app();
        app.world_mut().spawn((Obstacle { half_extents: Vec3::new(5.0, 0.1, 20.0) }, Transform::from_xyz(0.0, 0.1, -21.0)));
        let player = app
            .world_mut()
            .spawn((Locomotion::default(), MoveIntent { direction: Vec3::NEG_Z, ..default() }))
            .id();
        run(&mut app, 400);
        let pos = app.world().get::<Transform>(player).unwrap().translation;
        assert!((pos.y - 0.2).abs() < 0.08, "not standing on the curb: {pos}");
    }

    #[test]
    fn character_pushes_a_dynamic_crate() {
        let mut app = app();
        let crate_ = app
            .world_mut()
            .spawn((RigidBody::Dynamic, Collider::cuboid(0.4, 0.4, 0.4), ColliderMassProperties::Mass(30.0), Transform::from_xyz(0.0, 0.41, -1.5)))
            .id();
        app.world_mut().spawn((Locomotion::default(), MoveIntent { direction: Vec3::NEG_Z, ..default() }));
        run(&mut app, 300);
        let z = app.world().get::<Transform>(crate_).unwrap().translation.z;
        assert!(z < -3.0, "crate was not pushed: z = {z}");
    }

    #[test]
    fn running_into_prone_dives_and_keeps_momentum() {
        let mut app = app();
        let player = app
            .world_mut()
            .spawn((Locomotion::default(), MoveIntent { direction: Vec3::NEG_Z, ..default() }))
            .id();
        run(&mut app, 120);
        let before = app.world().get::<Transform>(player).unwrap().translation.z;
        app.world_mut().get_mut::<MoveIntent>(player).unwrap().prone = true;
        run(&mut app, 1);
        {
            let mut intent = app.world_mut().get_mut::<MoveIntent>(player).unwrap();
            intent.prone = false;
            intent.direction = Vec3::ZERO;
        }
        run(&mut app, 45); // ~0.7 s
        let w = app.world();
        assert!(matches!(w.get::<Locomotion>(player).unwrap().state, LocomotionState::Dive | LocomotionState::Grounded { .. }));
        let slid = before - w.get::<Transform>(player).unwrap().translation.z;
        assert!(slid > 1.8, "stopped dead instead of diving: slid {slid} m");
    }
}
