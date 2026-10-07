//! Velocity response, mirroring `MotionModel::Step_Stick`: velocity follows
//! the desired (stick) velocity through a critically damped spring, and the
//! resulting acceleration is clamped to a maximum.

use bevy::prelude::*;

/// Advances `velocity` toward `desired` by `dt`. `accel` is the spring's
/// acceleration state and must persist between steps. `max_accel <= 0`
/// disables the clamp.
pub fn step(
    velocity: &mut Vec3,
    accel: &mut Vec3,
    desired: Vec3,
    stiffness: f32,
    max_accel: f32,
    dt: f32,
) {
    let v0 = *velocity;
    let j0 = v0 - desired;
    let j1 = *accel + j0 * stiffness;
    let decay = (-stiffness * dt).exp();

    let mut v = desired + (j0 + j1 * dt) * decay;
    let mut a = (*accel - j1 * (stiffness * dt)) * decay;

    if max_accel > 0.0 && a.length_squared() > max_accel * max_accel {
        let to_desired = desired - v0;
        a = to_desired.normalize_or_zero() * max_accel;
        // Don't step past the target when the clamp kicks in.
        v = if to_desired.length() <= max_accel * dt { desired } else { v0 + a * dt };
    }

    *velocity = v;
    *accel = a;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converges_without_overshooting_much() {
        let (mut v, mut a) = (Vec3::ZERO, Vec3::ZERO);
        let target = Vec3::X * 4.0;
        let mut peak: f32 = 0.0;
        for _ in 0..240 {
            step(&mut v, &mut a, target, 10.0, 0.0, 1.0 / 60.0);
            peak = peak.max(v.x);
        }
        assert!(v.abs_diff_eq(target, 1e-3));
        assert!(peak <= 4.0 + 1e-3, "critically damped spring overshot: {peak}");
    }

    #[test]
    fn acceleration_is_clamped() {
        let (mut v, mut a) = (Vec3::ZERO, Vec3::ZERO);
        let dt = 1.0 / 60.0;
        step(&mut v, &mut a, Vec3::X * 6.0, 30.0, 10.0, dt);
        assert!(a.length() <= 10.0 + 1e-4);
        assert!((v.x - 10.0 * dt).abs() < 1e-5);
    }
}
