//! Sprint stamina, following the game's `PlayerSprintController`: stamina
//! out of 100 drains while sprinting and moving, regenerates after a delay,
//! and once exhausted sprinting is locked until it refills to a threshold.

use bevy::prelude::*;

use super::components::MoveIntent;
use super::state::{Gait, Locomotion, LocomotionState};
use super::tuning::LocomotionTuning;

/// The game's "Infinite Sprint" cheat writes 100.0 here, so that's the cap.
pub const STAMINA_MAX: f32 = 100.0;

#[derive(Component, Debug, Clone, Copy)]
pub struct SprintStamina {
    pub value: f32,
    pub regen_delay: f32,
    pub exhausted: bool,
    pub sprint_time: f32,
    pub non_sprint_time: f32,
}

impl Default for SprintStamina {
    fn default() -> Self {
        Self { value: STAMINA_MAX, regen_delay: 0.0, exhausted: false, sprint_time: 0.0, non_sprint_time: 0.0 }
    }
}

impl SprintStamina {
    pub fn can_sprint(&self) -> bool {
        !self.exhausted
    }

    pub fn tick(&mut self, sprinting: bool, moving: bool, dt: f32, t: &LocomotionTuning) {
        if sprinting {
            self.sprint_time += dt;
            self.non_sprint_time = 0.0;
            if t.stamina_drain_enabled && moving {
                self.value = (self.value - t.stamina_drain * dt).max(0.0);
            }
            self.regen_delay = t.stamina_regen_delay;
            if self.value <= 0.0 {
                self.exhausted = true;
            }
        } else {
            self.non_sprint_time += dt;
            self.sprint_time = 0.0;
            if self.regen_delay > 0.0 {
                self.regen_delay -= dt;
            } else {
                self.value = (self.value + t.stamina_regen * dt).min(STAMINA_MAX);
            }
        }
        if self.exhausted && self.value >= t.stamina_min_to_sprint {
            self.exhausted = false;
        }
    }
}

pub fn update_stamina(
    time: Res<Time>,
    mut query: Query<(&Locomotion, &MoveIntent, &LocomotionTuning, &mut SprintStamina)>,
) {
    let dt = time.delta_secs();
    for (loco, intent, tuning, mut stamina) in &mut query {
        let sprinting = matches!(loco.state, LocomotionState::Grounded { gait: Gait::Sprint, .. });
        let moving = intent.direction.length() >= tuning.move_deadzone;
        stamina.tick(sprinting, moving, dt, tuning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhaustion_locks_sprint_until_threshold() {
        let t = LocomotionTuning { stamina_drain_enabled: true, ..default() };
        let mut s = SprintStamina::default();
        for _ in 0..600 {
            s.tick(true, true, 0.1, &t);
        }
        assert_eq!(s.value, 0.0);
        assert!(!s.can_sprint());

        // Regen waits for the delay, then refills until sprint unlocks.
        s.tick(false, true, 0.5, &t);
        assert_eq!(s.value, 0.0);
        let mut elapsed = 0.5;
        while !s.can_sprint() {
            s.tick(false, true, 0.1, &t);
            elapsed += 0.1;
        }
        let expected = t.stamina_regen_delay + t.stamina_min_to_sprint / t.stamina_regen;
        assert!((elapsed - expected).abs() < 0.25, "unlocked after {elapsed}s");
    }

    #[test]
    fn drain_is_off_by_default() {
        let t = LocomotionTuning::default();
        let mut s = SprintStamina::default();
        s.tick(true, true, 10.0, &t);
        assert_eq!(s.value, STAMINA_MAX);
    }
}
