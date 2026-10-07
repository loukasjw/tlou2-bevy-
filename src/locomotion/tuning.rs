use bevy::prelude::*;

use super::state::{Gait, LandingKind, Stance};
use super::traversal::{EntryGait, HeightBucket, LedgeHeight, TraversalKind};

/// Per-character movement tuning. A component (not a resource) so NPCs and
/// the player can feel different while sharing the same systems.
///
/// The structure follows the game's DC settings structs, but the numbers are
/// estimates: the decompiled dump carries no data section, and the original
/// values live in DC script data anyway.
///
/// Every value below is `placeholder (measure)` unless tagged otherwise.
/// The research pass (`ghidra tlou decoding\answer.md`) found none of them as
/// readable data: speeds come from clip root motion x `player-anim-speed-mult`,
/// and the remaining tunables sit in DC structs with unnamed fields. Measure
/// in-game (60 fps capture, count frames) and note the source next to the value.
#[derive(Component, Debug, Clone)]
pub struct LocomotionTuning {
    // Ground speeds (m/s).
    pub walk_speed: f32,
    pub run_speed: f32,
    pub sprint_speed: f32,
    pub crouch_walk_speed: f32,
    pub crouch_run_speed: f32,
    pub crawl_speed: f32,

    // Motion model (critically damped spring stiffness, 1/s).
    // confirmed: `UpdateMovement` smooths with exp(-k*dt), k remapped from the
    // physics weight w in 0..0.25 onto 20..1000 /s (exe 142af9618/142affe74/142af9630).
    // guess: k = 20 smooths clip velocity that already ramps up, so it is not
    // a stick-response stiffness. These values stand in for clip ramp + smoothing.
    pub accel_stiffness: f32,
    pub decel_stiffness: f32,
    pub max_accel: f32,
    pub air_stiffness: f32,
    pub turn_rate: f32,

    // Airborne.
    pub gravity: f32,
    /// Gravity multiplier once descending, for a snappier arc.
    pub fall_gravity_scale: f32,
    pub jump_speed: f32,
    /// Ignore ground contact this long after takeoff so we don't "land" on frame 1.
    pub min_jump_time: f32,
    pub coyote_time: f32,
    pub jump_buffer_time: f32,

    // Analog stick thresholds on |direction|.
    pub move_deadzone: f32,
    pub run_threshold: f32,

    // Capsule.
    pub radius: f32,
    pub standing_height: f32,
    pub crouching_height: f32,
    pub prone_height: f32,
    /// Ledges up to this height are walked onto, not collided with.
    pub step_height: f32,
    /// How far ahead to look for low ceilings when auto-crouching.
    pub tunnel_probe_distance: f32,

    // Stance change durations (s).
    pub stand_crouch_time: f32,
    pub crouch_prone_time: f32,
    pub stand_prone_time: f32,

    // Dive to prone (going prone while sprinting).
    pub dive_min_speed: f32,
    pub dive_speed: f32,
    pub dive_duration: f32,
    pub dive_friction: f32,

    // Evade (dodge). The game picks an evade melee move relative to nearby
    // targets (`PlayerMeleeController::TryEvade`, confirmed); no timing data found.
    pub evade_speed: f32,
    pub evade_duration: f32,
    pub evade_friction: f32,

    // Traversal.
    pub traversal_probe_distance: f32,
    pub vault_min_height: f32,
    pub vault_max_height: f32,
    /// Obstacles deeper than this are climbed onto, not vaulted.
    pub vault_max_depth: f32,
    pub vault_clearance: f32,
    pub half_meter_max_height: f32,
    pub climb_max_height: f32,
    /// How long to push into a low ledge before auto-climbing it.
    pub auto_climb_push_time: f32,
    pub ledge_catch_min_height: f32,
    pub ledge_catch_max_height: f32,

    // Landing.
    // confirmed (clip names, `player-fall.bin`): the game has more tiers than
    // these three: extra-soft, soft, medium, roll, hard-roll, extra-hard, plus
    // fall death. Thresholds not found.
    pub hard_landing_height: f32,
    pub heavy_landing_height: f32,
    pub moving_landing_min_stick: f32,
    /// Min cosine between the stick and the stick/momentum blend.
    pub moving_landing_min_dot: f32,

    // Sprint stamina (out of `sprint::STAMINA_MAX`).
    // guess: no stamina tunable was found in exe strings or data, which is
    // weak evidence that player sprint is unlimited; drain stays off.
    pub stamina_drain_enabled: bool,
    pub stamina_drain: f32,
    pub stamina_regen: f32,
    pub stamina_regen_delay: f32,
    pub stamina_min_to_sprint: f32,
}

impl Default for LocomotionTuning {
    fn default() -> Self {
        Self {
            walk_speed: 1.6,
            run_speed: 3.8,
            sprint_speed: 6.0,
            crouch_walk_speed: 1.1,
            crouch_run_speed: 2.4,
            crawl_speed: 0.6,

            accel_stiffness: 9.0,
            decel_stiffness: 12.0,
            max_accel: 30.0,
            air_stiffness: 0.8,
            turn_rate: 12.0,

            gravity: 22.0,
            fall_gravity_scale: 1.6,
            jump_speed: 5.5,
            min_jump_time: 0.1,
            coyote_time: 0.12,
            jump_buffer_time: 0.15,

            move_deadzone: 0.15,
            run_threshold: 0.6,

            radius: 0.3,
            standing_height: 1.8,
            crouching_height: 1.1,
            prone_height: 0.5,
            step_height: 0.3,
            tunnel_probe_distance: 0.7,

            stand_crouch_time: 0.18,
            crouch_prone_time: 0.45,
            stand_prone_time: 0.65,

            dive_min_speed: 4.5,
            dive_speed: 5.5,
            dive_duration: 0.7,
            dive_friction: 3.0,

            evade_speed: 5.0,
            evade_duration: 0.45,
            evade_friction: 5.0,

            traversal_probe_distance: 1.0,
            vault_min_height: 0.4,
            vault_max_height: 1.2,
            vault_max_depth: 0.8,
            vault_clearance: 0.2,
            half_meter_max_height: 0.65,
            climb_max_height: 1.7,
            auto_climb_push_time: 0.12,
            ledge_catch_min_height: 0.3,
            ledge_catch_max_height: 1.5,

            hard_landing_height: 1.5,
            heavy_landing_height: 3.5,
            moving_landing_min_stick: 0.5,
            moving_landing_min_dot: 0.5,

            stamina_drain_enabled: false,
            stamina_drain: 20.0,
            stamina_regen: 25.0,
            stamina_regen_delay: 1.0,
            stamina_min_to_sprint: 25.0,
        }
    }
}

impl LocomotionTuning {
    pub fn speed(&self, stance: Stance, gait: Gait) -> f32 {
        match (stance, gait) {
            (_, Gait::Idle) => 0.0,
            (Stance::Standing, Gait::Walk) => self.walk_speed,
            (Stance::Standing, Gait::Run) => self.run_speed,
            (Stance::Standing, Gait::Sprint) => self.sprint_speed,
            (Stance::Crouching, Gait::Walk) => self.crouch_walk_speed,
            (Stance::Crouching, Gait::Run | Gait::Sprint) => self.crouch_run_speed,
            (Stance::Prone, _) => self.crawl_speed,
        }
    }

    pub fn height(&self, stance: Stance) -> f32 {
        match stance {
            Stance::Standing => self.standing_height,
            Stance::Crouching => self.crouching_height,
            Stance::Prone => self.prone_height,
        }
    }

    /// The tallest stance that fits under a ceiling `clearance` above the feet.
    pub fn tallest_stance_under(&self, clearance: f32) -> Stance {
        if clearance >= self.standing_height {
            Stance::Standing
        } else if clearance >= self.crouching_height {
            Stance::Crouching
        } else {
            Stance::Prone
        }
    }

    pub fn stance_change_time(&self, from: Stance, to: Stance) -> f32 {
        use Stance::*;
        match (from, to) {
            (Standing, Crouching) | (Crouching, Standing) => self.stand_crouch_time,
            (Crouching, Prone) | (Prone, Crouching) => self.crouch_prone_time,
            (Standing, Prone) | (Prone, Standing) => self.stand_prone_time,
            _ => 0.0,
        }
    }

    pub fn landing_kind(&self, fall_height: f32) -> LandingKind {
        if fall_height < self.hard_landing_height {
            LandingKind::Light
        } else if fall_height < self.heavy_landing_height {
            LandingKind::Hard
        } else {
            LandingKind::Heavy
        }
    }

    pub fn landing_recovery(&self, kind: LandingKind, moving: bool) -> f32 {
        match (kind, moving) {
            (LandingKind::Light, true) => 0.05,
            (LandingKind::Light, false) => 0.15,
            (LandingKind::Hard, true) => 0.25,
            (LandingKind::Hard, false) => 0.45,
            (LandingKind::Heavy, true) => 0.6,
            (LandingKind::Heavy, false) => 0.9,
        }
    }

    pub fn traversal_duration(&self, kind: TraversalKind, bucket: HeightBucket, entry: EntryGait) -> f32 {
        let base = match (kind, bucket) {
            (TraversalKind::Vault, HeightBucket::Half) => 0.45,
            (TraversalKind::Vault, HeightBucket::One) => 0.6,
            (TraversalKind::Vault, HeightBucket::OneAndHalf) => 0.75,
            (TraversalKind::HalfMeterClimb, _) => 0.4,
            (TraversalKind::ClimbUp, HeightBucket::OneAndHalf) => 0.95,
            (TraversalKind::ClimbUp, _) => 0.7,
            (TraversalKind::LedgeCatch(LedgeHeight::Knee), _) => 0.45,
            (TraversalKind::LedgeCatch(LedgeHeight::Waist), _) => 0.65,
            (TraversalKind::LedgeCatch(LedgeHeight::Chest), _) => 0.85,
        };
        let scale = match entry {
            EntryGait::Idle => 1.15,
            EntryGait::Walk => 1.0,
            EntryGait::Run => 0.85,
        };
        base * scale
    }
}
