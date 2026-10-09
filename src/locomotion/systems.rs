use bevy::prelude::*;
use bevy_rapier3d::prelude::{Collider, KinematicCharacterController};

use super::components::{
    EnvironmentSensors, GroundSensor, KinematicBody, LocomotionMemory, MoveIntent,
};
use super::motion_model;
use super::sprint::SprintStamina;
use super::state::{evaluate, LandingKind, Locomotion, LocomotionState, Stance, TransitionInput};
use super::tuning::LocomotionTuning;

pub fn update_state(
    time: Res<Time>,
    mut query: Query<(
        Entity,
        &mut Locomotion,
        &mut LocomotionMemory,
        &mut KinematicBody,
        &MoveIntent,
        &GroundSensor,
        &EnvironmentSensors,
        &LocomotionTuning,
        &SprintStamina,
        &Transform,
    )>,
) {
    let dt = time.delta_secs();
    for (entity, mut loco, mut memory, mut body, intent, ground, env, tuning, stamina, transform) in
        &mut query
    {
        memory.tick(dt, intent, ground, env, tuning);
        // Bypass so `Changed<Locomotion>` only fires on real transitions.
        loco.bypass_change_detection().time_in_state += dt;

        let input = TransitionInput {
            intent,
            ground,
            env,
            memory: &memory,
            tuning,
            position: transform.translation,
            velocity: body.velocity,
            can_sprint: stamina.can_sprint(),
        };
        let Some(next) = evaluate(&loco, &input) else { continue };

        debug!("{entity}: {:?} -> {next:?}", loco.state);
        on_enter(next, &mut body, &mut memory, tuning);
        loco.enter(next);
    }
}

/// One-shot side effects of entering a state.
fn on_enter(
    state: LocomotionState,
    body: &mut KinematicBody,
    memory: &mut LocomotionMemory,
    tuning: &LocomotionTuning,
) {
    match state {
        LocomotionState::Jump => {
            body.velocity.y = tuning.jump_speed;
            body.spring_accel = Vec3::ZERO;
            memory.jump_buffer = 0.0;
            memory.coyote = 0.0;
        }
        LocomotionState::Traversal(_) => {
            body.spring_accel = Vec3::ZERO;
            memory.jump_buffer = 0.0;
            memory.coyote = 0.0;
            memory.obstacle_push = 0.0;
        }
        LocomotionState::Evade { direction, .. } => {
            body.velocity = (direction * tuning.evade_speed).with_y(body.velocity.y);
            body.spring_accel = Vec3::ZERO;
            memory.jump_buffer = 0.0;
        }
        LocomotionState::Dive => {
            let dir = body.velocity.with_y(0.0).normalize_or_zero();
            let speed = body.velocity.xz().length().max(tuning.dive_speed);
            body.velocity = (dir * speed).with_y(body.velocity.y);
            body.spring_accel = Vec3::ZERO;
            memory.jump_buffer = 0.0;
            memory.auto_restore = None;
        }
        LocomotionState::ChangingStance { from, to, auto } => {
            // A jump press spent on standing up must not fire again on arrival.
            memory.jump_buffer = 0.0;
            memory.auto_restore = match (auto, memory.auto_restore) {
                (false, _) => None,
                (true, Some(restore)) if to >= restore => None,
                (true, Some(restore)) => Some(restore),
                (true, None) if to < from => Some(from),
                (true, None) => None,
            };
        }
        LocomotionState::Grounded { .. }
        | LocomotionState::Fall { .. }
        | LocomotionState::Landing { .. } => {}
    }
}

fn horizontal(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

/// Drive horizontal velocity toward `desired` through the motion model,
/// using the accel or decel stiffness depending on whether we're speeding up.
fn drive(body: &mut KinematicBody, desired: Vec3, stiffness: Option<f32>, t: &LocomotionTuning, dt: f32) {
    let mut v = horizontal(body.velocity);
    let mut a = horizontal(body.spring_accel);
    let k = stiffness.unwrap_or(if desired.length_squared() >= v.length_squared() {
        t.accel_stiffness
    } else {
        t.decel_stiffness
    });
    motion_model::step(&mut v, &mut a, desired, k, t.max_accel, dt);
    body.velocity = v.with_y(body.velocity.y);
    body.spring_accel = a;
}

fn apply_gravity(body: &mut KinematicBody, grounded: bool, t: &LocomotionTuning, dt: f32) {
    let v = &mut body.velocity.y;
    if grounded && *v <= 0.0 {
        *v = 0.0;
    } else {
        let scale = if *v < 0.0 { t.fall_gravity_scale } else { 1.0 };
        *v -= t.gravity * scale * dt;
    }
}

/// Per-state velocity. Every state writes velocity only; `integrate` is the
/// single place that moves the transform.
pub fn apply_movement(
    time: Res<Time>,
    mut query: Query<(
        &Locomotion,
        &MoveIntent,
        &GroundSensor,
        &LocomotionTuning,
        &Transform,
        &mut KinematicBody,
    )>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (loco, intent, ground, t, transform, mut body) in &mut query {
        let wish = horizontal(intent.direction).normalize_or_zero();
        match loco.state {
            LocomotionState::Grounded { stance, gait } => {
                drive(&mut body, wish * t.speed(stance, gait), None, t, dt);
                apply_gravity(&mut body, ground.grounded, t, dt);
            }
            LocomotionState::ChangingStance { .. } => {
                drive(&mut body, Vec3::ZERO, Some(t.stance_decel_stiffness), t, dt);
                apply_gravity(&mut body, ground.grounded, t, dt);
            }
            LocomotionState::Dive | LocomotionState::Evade { .. } => {
                // Low stiffness and no accel clamp: a slide that bleeds off.
                let friction = if loco.state == LocomotionState::Dive { t.dive_friction } else { t.evade_friction };
                let mut v = horizontal(body.velocity);
                let mut a = horizontal(body.spring_accel);
                motion_model::step(&mut v, &mut a, Vec3::ZERO, friction, 0.0, dt);
                body.velocity = v.with_y(body.velocity.y);
                body.spring_accel = a;
                apply_gravity(&mut body, ground.grounded, t, dt);
            }
            LocomotionState::Jump | LocomotionState::Fall { .. } => {
                // Momentum carries; the stick only nudges.
                let carried = horizontal(body.velocity).length().max(t.walk_speed);
                drive(&mut body, wish * carried, Some(t.air_stiffness), t, dt);
                apply_gravity(&mut body, false, t, dt);
            }
            LocomotionState::Landing { kind, moving } => {
                let target = if moving {
                    let speed = if kind == LandingKind::Light { t.run_speed } else { t.walk_speed };
                    wish * speed
                } else {
                    Vec3::ZERO
                };
                drive(&mut body, target, None, t, dt);
                apply_gravity(&mut body, ground.grounded, t, dt);
            }
            LocomotionState::Traversal(motion) => {
                // Kinematic: velocity that lands exactly on the curve this frame.
                body.velocity = (motion.sample(loco.time_in_state) - transform.translation) / dt;
            }
        }
    }
}

pub fn integrate(
    time: Res<Time>,
    mut query: Query<(
        &Locomotion,
        &MoveIntent,
        &KinematicBody,
        &GroundSensor,
        &LocomotionTuning,
        &mut Transform,
        &mut KinematicCharacterController,
    )>,
) {
    let dt = time.delta_secs();
    for (loco, intent, body, ground, tuning, mut transform, mut controller) in &mut query {
        let mut displacement = body.velocity * dt;
        if matches!(loco.state, LocomotionState::Traversal(_)) {
            // Scripted curve: bypasses collision.
            transform.translation += displacement;
        } else {
            // Rapier slides/steps the capsule and applies the result to the
            // transform in PostUpdate.
            // The capsule starts at step height: ledges below it are walked
            // onto (feet follow the sensed ground), anything taller is a wall.
            let span = (tuning.height(loco.state.stance()) - tuning.step_height).max(0.2);
            let radius = tuning.radius.min(span * 0.5);
            controller.custom_shape = Some((
                Collider::capsule_y((span * 0.5 - radius).max(0.0), radius),
                Vec3::Y * (tuning.step_height + span * 0.5),
                Quat::IDENTITY,
            ));
            // Never sink below the ground; rise onto low ledges.
            displacement.y = displacement.y.max(ground.height - transform.translation.y);
            controller.translation = Some(displacement);
        }

        // Strafing faces the aim on the ground; airborne and traversal moves
        // keep facing their motion.
        let strafe = match loco.state {
            LocomotionState::Grounded { .. }
            | LocomotionState::ChangingStance { .. }
            | LocomotionState::Landing { .. } => intent.strafe_facing.map(horizontal),
            _ => None,
        };
        // A dodge keeps facing the threat, whichever way it moves.
        let evading = matches!(loco.state, LocomotionState::Evade { .. });
        let flat = if evading { Vec3::ZERO } else { strafe.unwrap_or(horizontal(body.velocity)) };
        if flat.length_squared() > 0.01 {
            // Bevy's forward is -Z. Prone turns slowly, as in the game.
            let turn_rate = if loco.state.stance() == Stance::Prone { tuning.turn_rate * 0.3 } else { tuning.turn_rate };
            let target = Quat::from_rotation_y(f32::atan2(-flat.x, -flat.z));
            let t = 1.0 - (-turn_rate * dt).exp();
            transform.rotation = transform.rotation.slerp(target, t);
        }
    }
}
