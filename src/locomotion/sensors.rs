//! Stand-in physics: a flat floor at y = 0 plus axis-aligned boxes that can be
//! stood on, collided with, or crawled under. Replace with shape casts from a
//! physics backend (e.g. avian3d); the state machine only depends on the
//! sensor components, not on this module.

use bevy::prelude::*;

use super::components::{EnvironmentSensors, GroundSensor, KinematicBody, MoveIntent};
use super::state::{Locomotion, LocomotionState};
use super::traversal::TraversalCandidate;
use super::tuning::LocomotionTuning;

const FLOOR_Y: f32 = 0.0;
const GROUND_SKIN: f32 = 0.02;
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

/// Highest walkable surface under `p` that is no more than a step above `feet_y`.
fn ground_height_at(boxes: &[Aabb], p: Vec2, feet_y: f32, t: &LocomotionTuning) -> f32 {
    boxes
        .iter()
        .filter(|b| b.footprint_contains(p, 0.5 * t.radius) && b.max.y <= feet_y + t.step_height)
        .map(|b| b.max.y)
        .fold(FLOOR_Y, f32::max)
}

/// Clearance from `feet_y` to the lowest overhead box above `p`.
fn ceiling_at(boxes: &[Aabb], p: Vec2, feet_y: f32, t: &LocomotionTuning) -> f32 {
    boxes
        .iter()
        .filter(|b| b.footprint_contains(p, t.radius) && b.min.y > feet_y + t.step_height)
        .map(|b| b.min.y - feet_y)
        .fold(f32::INFINITY, f32::min)
}

pub fn sense_environment(
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
    let boxes = collect(&obstacles);
    for (transform, body, intent, t, mut ground, mut env) in &mut characters {
        let pos = transform.translation;
        ground.height = ground_height_at(&boxes, pos.xz(), pos.y, t);
        ground.grounded = pos.y <= ground.height + GROUND_SKIN && body.velocity.y <= 0.0;

        let dir = intent.direction.xz().normalize_or_zero();
        let moving = intent.direction.length() >= t.move_deadzone;
        env.ceiling_height = ceiling_at(&boxes, pos.xz(), pos.y, t);
        env.ceiling_ahead = if moving {
            ceiling_at(&boxes, pos.xz() + dir * t.tunnel_probe_distance, pos.y, t)
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
                over: at(exit + LANDING_MARGIN, ground_height_at(boxes, over_xz, pos.y, t)),
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

/// Pushes characters out of boxes they overlap vertically, steps them up low
/// ledges and keeps them on the ground. Skipped mid-traversal (kinematic).
pub fn resolve_collisions(
    obstacles: Query<(&Transform, &Obstacle)>,
    mut characters: Query<
        (&Locomotion, &LocomotionTuning, &mut Transform, &mut KinematicBody),
        Without<Obstacle>,
    >,
) {
    let boxes = collect(&obstacles);
    for (loco, t, mut transform, mut body) in &mut characters {
        if matches!(loco.state, LocomotionState::Traversal(_)) {
            continue;
        }
        let height = t.height(loco.state.stance());
        let mut pos = transform.translation;
        for b in &boxes {
            let walkable = b.max.y - pos.y <= t.step_height;
            let overhead = b.min.y >= pos.y + height;
            if walkable || overhead {
                continue;
            }
            let center = Vec2::new(b.min.x + b.max.x, b.min.z + b.max.z) * 0.5;
            let half = Vec2::new(b.max.x - b.min.x, b.max.z - b.min.z) * 0.5 + Vec2::splat(t.radius);
            let delta = pos.xz() - center;
            let penetration = half - delta.abs();
            if penetration.min_element() <= 0.0 {
                continue;
            }
            // Push out along the axis of least penetration and kill velocity into the wall.
            if penetration.x < penetration.y {
                pos.x += penetration.x * delta.x.signum();
                body.velocity.x = 0.0;
            } else {
                pos.z += penetration.y * delta.y.signum();
                body.velocity.z = 0.0;
            }
        }

        let ground = ground_height_at(&boxes, pos.xz(), pos.y, t);
        if pos.y < ground {
            pos.y = ground;
            body.velocity.y = body.velocity.y.max(0.0);
        }
        transform.translation = pos;
    }
}
