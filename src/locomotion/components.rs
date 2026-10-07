use bevy::prelude::*;

use super::state::Stance;
use super::traversal::TraversalCandidate;
use super::tuning::LocomotionTuning;

/// What the controller wants to do this frame. Written by player input or AI;
/// the locomotion systems never read devices directly.
#[derive(Component, Debug, Default, Clone)]
pub struct MoveIntent {
    /// World-space horizontal direction, length in `0..=1` (analog magnitude).
    pub direction: Vec3,
    pub sprint: bool,
    /// Edge-triggered: true only on the frame the action fires.
    pub jump: bool,
    /// Crouch toggle (a tap of the crouch button).
    pub crouch: bool,
    /// Prone toggle (holding the crouch button).
    pub prone: bool,
    /// Strafe (the game's aim locomotion, `player-strafe.bin`): face this
    /// world direction instead of the velocity, and move no faster than a walk.
    pub strafe_facing: Option<Vec3>,
    /// Edge-triggered dodge in this world direction (the game's evade).
    pub evade: Option<Vec3>,
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct KinematicBody {
    pub velocity: Vec3,
    /// Motion-model spring state; see `motion_model::step`.
    pub spring_accel: Vec3,
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct GroundSensor {
    pub grounded: bool,
    /// World height of the surface under the feet.
    pub height: f32,
}

/// Spatial queries the state machine needs, filled by the physics layer.
#[derive(Component, Debug, Clone, Copy)]
pub struct EnvironmentSensors {
    /// Clearance from the feet to the ceiling right here.
    pub ceiling_height: f32,
    /// Clearance a short way ahead along the wish direction.
    pub ceiling_ahead: f32,
    /// Closest traversable edge ahead, if any.
    pub traversal: Option<TraversalCandidate>,
}

impl Default for EnvironmentSensors {
    fn default() -> Self {
        Self { ceiling_height: f32::INFINITY, ceiling_ahead: f32::INFINITY, traversal: None }
    }
}

/// Timers and flags that outlive a single state.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct LocomotionMemory {
    /// > 0 while a jump press is buffered.
    pub jump_buffer: f32,
    /// > 0 while we may still jump after leaving the ground.
    pub coyote: f32,
    /// How long we've been pushing into a traversable edge.
    pub obstacle_push: f32,
    /// Stance to return to once a low ceiling has been cleared.
    pub auto_restore: Option<Stance>,
}

impl LocomotionMemory {
    pub fn tick(
        &mut self,
        dt: f32,
        intent: &MoveIntent,
        ground: &GroundSensor,
        env: &EnvironmentSensors,
        t: &LocomotionTuning,
    ) {
        self.jump_buffer = if intent.jump { t.jump_buffer_time } else { (self.jump_buffer - dt).max(0.0) };
        self.coyote = if ground.grounded { t.coyote_time } else { (self.coyote - dt).max(0.0) };
        let pushing = env.traversal.is_some() && intent.direction.length() >= t.move_deadzone;
        self.obstacle_push = if pushing { self.obstacle_push + dt } else { 0.0 };
    }
}
