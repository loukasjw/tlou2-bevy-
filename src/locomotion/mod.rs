//! Character locomotion modeled on The Last of Us Part II's player movement:
//! a pure state machine (`state`) driven by intent and sensor components,
//! a motion-model velocity spring, edge traversal and sprint stamina.
//! Systems run Input -> Sense -> Transition -> Movement -> Integrate.

pub mod components;
pub mod motion_model;
pub mod sensors;
pub mod sprint;
pub mod state;
pub mod systems;
pub mod traversal;
pub mod tuning;

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocomotionSet {
    /// Game code writes `MoveIntent` here (player input, AI).
    Input,
    Sense,
    Transition,
    Movement,
    Integrate,
}

pub struct LocomotionPlugin;

impl Plugin for LocomotionPlugin {
    fn build(&self, app: &mut App) {
        use LocomotionSet::*;
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
            .configure_sets(Update, (Input, Sense, Transition, Movement, Integrate).chain())
            .add_systems(
                Update,
                (
                    (sensors::init_controllers, sensors::add_obstacle_colliders).before(Sense),
                    (sensors::absorb_blocked_motion, sensors::sense_environment).chain().in_set(Sense),
                    (sprint::update_stamina, systems::update_state).chain().in_set(Transition),
                    systems::apply_movement.in_set(Movement),
                    systems::integrate.in_set(Integrate),
                ),
            );
    }
}
