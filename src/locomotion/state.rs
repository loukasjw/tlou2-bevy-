//! The locomotion state machine. `evaluate` is a pure function of the current
//! state plus a snapshot of inputs/sensors, so every transition rule can be
//! unit-tested without spinning up an ECS world.

use bevy::prelude::*;

use super::components::{
    EnvironmentSensors, GroundSensor, KinematicBody, LocomotionMemory, MoveIntent,
};
use super::sprint::SprintStamina;
use super::traversal::{self, EntryGait, TraversalKind, TraversalMotion};
use super::tuning::LocomotionTuning;

/// Ordered low to high, so stances compare by height.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stance {
    Prone,
    Crouching,
    #[default]
    Standing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Gait {
    #[default]
    Idle,
    Walk,
    Run,
    Sprint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LandingKind {
    Light,
    Hard,
    Heavy,
}

/// Stance and gait are orthogonal data inside `Grounded` rather than peer
/// states, so crouch-walk / crawl don't multiply the state count.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LocomotionState {
    Grounded { stance: Stance, gait: Gait },
    /// Timed stance change; movement is locked meanwhile. `auto` marks changes
    /// forced by a low ceiling rather than requested by the player.
    ChangingStance { from: Stance, to: Stance, auto: bool },
    /// Going prone at sprint speed: slide forward onto the belly.
    Dive,
    /// A quick dodge step, keeping the stance it started in and the facing.
    Evade { stance: Stance, direction: Vec3 },
    Jump,
    Fall { apex_y: f32 },
    Landing { kind: LandingKind, moving: bool },
    Traversal(TraversalMotion),
}

impl Default for LocomotionState {
    fn default() -> Self {
        Self::Grounded { stance: Stance::Standing, gait: Gait::Idle }
    }
}

impl LocomotionState {
    /// The stance whose capsule height applies right now.
    pub fn stance(&self) -> Stance {
        match *self {
            Self::Grounded { stance, .. } => stance,
            Self::ChangingStance { to, .. } => to,
            Self::Dive => Stance::Prone,
            Self::Evade { stance, .. } => stance,
            Self::Landing { kind: LandingKind::Hard | LandingKind::Heavy, .. } => Stance::Crouching,
            Self::Jump | Self::Fall { .. } | Self::Landing { .. } | Self::Traversal(_) => Stance::Standing,
        }
    }
}

#[derive(Component, Debug, Default, Clone)]
#[require(
    MoveIntent,
    LocomotionMemory,
    KinematicBody,
    GroundSensor,
    EnvironmentSensors,
    LocomotionTuning,
    SprintStamina,
    Transform
)]
pub struct Locomotion {
    pub state: LocomotionState,
    pub previous: LocomotionState,
    pub time_in_state: f32,
}

impl Locomotion {
    pub fn enter(&mut self, next: LocomotionState) {
        self.previous = self.state;
        self.state = next;
        self.time_in_state = 0.0;
    }
}

/// Read-only snapshot fed to `evaluate`.
pub struct TransitionInput<'a> {
    pub intent: &'a MoveIntent,
    pub ground: &'a GroundSensor,
    pub env: &'a EnvironmentSensors,
    pub memory: &'a LocomotionMemory,
    pub tuning: &'a LocomotionTuning,
    pub position: Vec3,
    pub velocity: Vec3,
    pub can_sprint: bool,
}

impl TransitionInput<'_> {
    fn fits(&self, stance: Stance) -> bool {
        self.env.ceiling_height >= self.tuning.height(stance)
    }

    fn lost_ground(&self) -> bool {
        !self.ground.grounded && self.memory.coyote <= 0.0
    }

    fn stick(&self) -> f32 {
        self.intent.direction.length()
    }

    fn moving(&self) -> bool {
        self.stick() >= self.tuning.move_deadzone
    }

    fn horizontal_speed(&self) -> f32 {
        self.velocity.xz().length()
    }

    fn fall(&self) -> LocomotionState {
        LocomotionState::Fall { apex_y: self.position.y }
    }
}

/// Returns the state to switch to, or `None` to stay. Rules are checked in
/// priority order; the first match wins.
pub fn evaluate(loco: &Locomotion, i: &TransitionInput) -> Option<LocomotionState> {
    use LocomotionState::*;
    let t = i.tuning;
    match loco.state {
        Grounded { stance, gait } => evaluate_grounded(stance, gait, i),
        ChangingStance { from, to, .. } => {
            if i.lost_ground() {
                Some(i.fall())
            } else {
                (loco.time_in_state >= t.stance_change_time(from, to))
                    .then_some(Grounded { stance: to, gait: Gait::Idle })
            }
        }
        Dive => {
            if i.lost_ground() {
                Some(i.fall())
            } else {
                (loco.time_in_state >= t.dive_duration)
                    .then_some(Grounded { stance: Stance::Prone, gait: Gait::Idle })
            }
        }
        Evade { stance, .. } => {
            if i.lost_ground() {
                Some(i.fall())
            } else {
                (loco.time_in_state >= t.evade_duration)
                    .then(|| Grounded { stance, gait: desired_gait(stance, i) })
            }
        }
        Jump => {
            if let Some(motion) = ledge_catch(i) {
                Some(Traversal(motion))
            } else if i.velocity.y <= 0.0 {
                Some(i.fall())
            } else if i.ground.grounded && loco.time_in_state >= t.min_jump_time {
                Some(Landing { kind: LandingKind::Light, moving: moving_landing(i) })
            } else {
                None
            }
        }
        Fall { apex_y } => {
            if let Some(motion) = ledge_catch(i) {
                Some(Traversal(motion))
            } else {
                i.ground.grounded.then(|| Landing {
                    kind: t.landing_kind(apex_y - i.position.y),
                    moving: moving_landing(i),
                })
            }
        }
        Landing { kind, moving } => {
            if i.lost_ground() {
                Some(i.fall())
            } else {
                (loco.time_in_state >= t.landing_recovery(kind, moving)).then(|| stand_up(i))
            }
        }
        Traversal(motion) => (loco.time_in_state >= motion.duration)
            .then(|| if i.ground.grounded { stand_up(i) } else { i.fall() }),
    }
}

fn evaluate_grounded(stance: Stance, gait: Gait, i: &TransitionInput) -> Option<LocomotionState> {
    use LocomotionState::*;
    use Stance::*;
    let t = i.tuning;

    if i.lost_ground() {
        return Some(i.fall());
    }

    if let Some(to) = auto_stance(stance, i) {
        return Some(ChangingStance { from: stance, to, auto: true });
    }

    if let Some(direction) = i.intent.evade.filter(|_| stance != Prone) {
        return Some(Evade { stance, direction: direction.with_y(0.0).normalize_or_zero() });
    }

    // Jump means "get higher": traverse or jump when standing, otherwise rise one stance.
    let mut request = None;
    if i.memory.jump_buffer > 0.0 {
        match stance {
            Standing => {
                if let Some(motion) = grounded_traversal(i) {
                    return Some(Traversal(motion));
                }
                if i.memory.coyote > 0.0 {
                    return Some(Jump);
                }
            }
            Crouching => request = Some(Standing),
            Prone => request = Some(Crouching),
        }
    }

    // Low ledges are climbed just by pushing into them.
    if stance == Standing && i.memory.obstacle_push >= t.auto_climb_push_time {
        if let Some(motion) = grounded_traversal(i).filter(|m| m.kind == TraversalKind::HalfMeterClimb) {
            return Some(Traversal(motion));
        }
    }

    if i.intent.prone {
        if stance == Prone {
            request = Some(Crouching);
        } else if stance == Standing && i.horizontal_speed() >= t.dive_min_speed {
            return Some(Dive);
        } else {
            request = Some(Prone);
        }
    } else if i.intent.crouch {
        request = Some(if stance == Crouching { Standing } else { Crouching });
    } else if stance != Standing && i.intent.sprint && i.can_sprint && i.stick() >= t.run_threshold {
        // Sprint raises one stance at a time: prone -> crouch -> stand.
        request = Some(if stance == Prone { Crouching } else { Standing });
    }

    if let Some(to) = request.filter(|&to| to != stance && i.fits(to)) {
        return Some(ChangingStance { from: stance, to, auto: false });
    }

    let desired = desired_gait(stance, i);
    (desired != gait).then_some(Grounded { stance, gait: desired })
}

/// Tunnel handling (the game's `InTunnelProbe`): drop to whatever fits under
/// the ceiling here or just ahead, and stand back up once clear if the
/// lowering was automatic.
fn auto_stance(stance: Stance, i: &TransitionInput) -> Option<Stance> {
    let ahead = if i.moving() { i.env.ceiling_ahead } else { f32::INFINITY };
    let fitting = i.tuning.tallest_stance_under(i.env.ceiling_height.min(ahead));
    if fitting < stance {
        return Some(fitting);
    }
    let restore = i.memory.auto_restore?;
    let target = fitting.min(restore);
    (target > stance).then_some(target)
}

pub fn desired_gait(stance: Stance, i: &TransitionInput) -> Gait {
    let magnitude = i.stick();
    if magnitude < i.tuning.move_deadzone {
        Gait::Idle
    } else if stance == Stance::Prone
        || magnitude < i.tuning.run_threshold
        || i.intent.strafe_facing.is_some()
    {
        Gait::Walk
    } else if i.intent.sprint && i.can_sprint && stance == Stance::Standing {
        Gait::Sprint
    } else {
        Gait::Run
    }
}

fn stand_up(i: &TransitionInput) -> LocomotionState {
    LocomotionState::Grounded { stance: Stance::Standing, gait: desired_gait(Stance::Standing, i) }
}

/// From `GroundLanding::ShouldTransitionToMovingLanding`: land into a run when
/// the stick is held and roughly agrees with the stick/momentum blend.
pub fn moving_landing(i: &TransitionInput) -> bool {
    let stick = i.intent.direction.with_y(0.0);
    if stick.length() < i.tuning.moving_landing_min_stick {
        return false;
    }
    let blended = (stick * 0.7 + i.velocity.with_y(0.0)).normalize_or_zero();
    stick.normalize_or_zero().dot(blended) > i.tuning.moving_landing_min_dot
}

fn entry_gait(i: &TransitionInput) -> EntryGait {
    EntryGait::from_speed(i.horizontal_speed(), i.tuning)
}

fn grounded_traversal(i: &TransitionInput) -> Option<TraversalMotion> {
    let candidate = i.env.traversal?;
    let kind = traversal::classify_grounded(&candidate, i.tuning)?;
    Some(traversal::plan(kind, &candidate, i.position, entry_gait(i), i.tuning))
}

fn ledge_catch(i: &TransitionInput) -> Option<TraversalMotion> {
    if !i.moving() {
        return None;
    }
    let candidate = i.env.traversal?;
    let kind = traversal::classify_airborne(&candidate, i.tuning)?;
    Some(traversal::plan(kind, &candidate, i.position, entry_gait(i), i.tuning))
}

#[cfg(test)]
mod tests {
    use super::super::traversal::TraversalCandidate;
    use super::*;

    struct Fixture {
        intent: MoveIntent,
        ground: GroundSensor,
        env: EnvironmentSensors,
        memory: LocomotionMemory,
        tuning: LocomotionTuning,
        position: Vec3,
        velocity: Vec3,
        can_sprint: bool,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                intent: MoveIntent::default(),
                ground: GroundSensor { grounded: true, height: 0.0 },
                env: EnvironmentSensors::default(),
                memory: LocomotionMemory { coyote: 0.1, ..default() },
                tuning: LocomotionTuning::default(),
                position: Vec3::ZERO,
                velocity: Vec3::ZERO,
                can_sprint: true,
            }
        }

        fn eval(&self, state: LocomotionState, time_in_state: f32) -> Option<LocomotionState> {
            let loco = Locomotion { state, previous: state, time_in_state };
            evaluate(
                &loco,
                &TransitionInput {
                    intent: &self.intent,
                    ground: &self.ground,
                    env: &self.env,
                    memory: &self.memory,
                    tuning: &self.tuning,
                    position: self.position,
                    velocity: self.velocity,
                    can_sprint: self.can_sprint,
                },
            )
        }
    }

    fn grounded(stance: Stance, gait: Gait) -> LocomotionState {
        LocomotionState::Grounded { stance, gait }
    }

    fn change(from: Stance, to: Stance, auto: bool) -> LocomotionState {
        LocomotionState::ChangingStance { from, to, auto }
    }

    fn ledge(height: f32, depth: f32) -> TraversalCandidate {
        TraversalCandidate {
            height,
            depth,
            edge: Vec3::new(0.0, height, -0.5),
            onto: Vec3::new(0.0, height, -0.9),
            over: Vec3::new(0.0, 0.0, -1.2),
        }
    }

    #[test]
    fn gait_follows_input_magnitude_and_stamina() {
        let mut f = Fixture::new();
        let idle = grounded(Stance::Standing, Gait::Idle);
        assert_eq!(f.eval(idle, 0.0), None);

        f.intent.direction = Vec3::X * 0.4;
        assert_eq!(f.eval(idle, 0.0), Some(grounded(Stance::Standing, Gait::Walk)));

        f.intent.direction = Vec3::X;
        f.intent.sprint = true;
        assert_eq!(f.eval(idle, 0.0), Some(grounded(Stance::Standing, Gait::Sprint)));

        f.can_sprint = false;
        assert_eq!(f.eval(idle, 0.0), Some(grounded(Stance::Standing, Gait::Run)));

        f.can_sprint = true;
        f.intent.strafe_facing = Some(Vec3::NEG_Z);
        assert_eq!(f.eval(idle, 0.0), Some(grounded(Stance::Standing, Gait::Walk)));
    }

    #[test]
    fn crouch_tap_toggles_and_hold_goes_prone() {
        let mut f = Fixture::new();
        f.intent.crouch = true;
        assert_eq!(
            f.eval(grounded(Stance::Standing, Gait::Idle), 0.0),
            Some(change(Stance::Standing, Stance::Crouching, false))
        );
        assert_eq!(
            f.eval(grounded(Stance::Crouching, Gait::Idle), 0.0),
            Some(change(Stance::Crouching, Stance::Standing, false))
        );

        f.intent.crouch = false;
        f.intent.prone = true;
        assert_eq!(
            f.eval(grounded(Stance::Standing, Gait::Idle), 0.0),
            Some(change(Stance::Standing, Stance::Prone, false))
        );
    }

    #[test]
    fn prone_at_sprint_speed_dives() {
        let mut f = Fixture::new();
        f.intent.prone = true;
        f.velocity = Vec3::X * 6.0;
        assert_eq!(f.eval(grounded(Stance::Standing, Gait::Sprint), 0.0), Some(LocomotionState::Dive));
        assert_eq!(
            f.eval(LocomotionState::Dive, 1.0),
            Some(grounded(Stance::Prone, Gait::Idle))
        );
    }

    #[test]
    fn evade_keeps_stance_and_ends_on_time() {
        let mut f = Fixture::new();
        f.intent.evade = Some(Vec3::new(2.0, 0.0, 0.0));
        let evade = LocomotionState::Evade { stance: Stance::Crouching, direction: Vec3::X };
        assert_eq!(f.eval(grounded(Stance::Crouching, Gait::Idle), 0.0), Some(evade));
        assert_eq!(f.eval(grounded(Stance::Prone, Gait::Idle), 0.0), None);

        f.intent.evade = None;
        assert_eq!(f.eval(evade, 0.1), None);
        assert_eq!(f.eval(evade, 1.0), Some(grounded(Stance::Crouching, Gait::Idle)));
    }

    #[test]
    fn sprint_raises_one_stance_at_a_time() {
        let mut f = Fixture::new();
        f.intent.direction = Vec3::X;
        f.intent.sprint = true;
        assert_eq!(
            f.eval(grounded(Stance::Prone, Gait::Walk), 0.0),
            Some(change(Stance::Prone, Stance::Crouching, false))
        );
        assert_eq!(
            f.eval(grounded(Stance::Crouching, Gait::Run), 0.0),
            Some(change(Stance::Crouching, Stance::Standing, false))
        );
    }

    #[test]
    fn low_ceiling_blocks_standing() {
        let mut f = Fixture::new();
        f.env.ceiling_height = 1.2;
        f.intent.crouch = true;
        assert_eq!(f.eval(grounded(Stance::Crouching, Gait::Idle), 0.0), None);
    }

    #[test]
    fn tunnel_ahead_auto_crouches_then_restores() {
        let mut f = Fixture::new();
        f.intent.direction = Vec3::X;
        f.env.ceiling_ahead = 1.3;
        assert_eq!(
            f.eval(grounded(Stance::Standing, Gait::Run), 0.0),
            Some(change(Stance::Standing, Stance::Crouching, true))
        );

        f.env.ceiling_ahead = 0.8;
        assert_eq!(
            f.eval(grounded(Stance::Crouching, Gait::Run), 0.0),
            Some(change(Stance::Crouching, Stance::Prone, true))
        );

        f.env.ceiling_ahead = f32::INFINITY;
        f.memory.auto_restore = Some(Stance::Standing);
        assert_eq!(
            f.eval(grounded(Stance::Prone, Gait::Walk), 0.0),
            Some(change(Stance::Prone, Stance::Standing, true))
        );
    }

    #[test]
    fn jump_from_prone_rises_to_crouch() {
        let mut f = Fixture::new();
        f.memory.jump_buffer = 0.1;
        assert_eq!(
            f.eval(grounded(Stance::Prone, Gait::Idle), 0.0),
            Some(change(Stance::Prone, Stance::Crouching, false))
        );
    }

    #[test]
    fn coyote_time_allows_late_jump_then_falls() {
        let mut f = Fixture::new();
        f.ground.grounded = false;
        let standing = grounded(Stance::Standing, Gait::Idle);
        assert_eq!(f.eval(standing, 0.0), None);

        f.memory.jump_buffer = 0.1;
        assert_eq!(f.eval(standing, 0.0), Some(LocomotionState::Jump));

        f.memory = LocomotionMemory::default();
        assert_eq!(f.eval(standing, 0.0), Some(LocomotionState::Fall { apex_y: 0.0 }));
    }

    #[test]
    fn jump_button_picks_traversal_by_shape() {
        let mut f = Fixture::new();
        f.memory.jump_buffer = 0.1;
        f.intent.direction = Vec3::NEG_Z;
        let standing = grounded(Stance::Standing, Gait::Run);
        let kind = |f: &Fixture| match f.eval(standing, 0.0) {
            Some(LocomotionState::Traversal(m)) => Some(m.kind),
            _ => None,
        };

        f.env.traversal = Some(ledge(0.9, 0.3));
        assert_eq!(kind(&f), Some(TraversalKind::Vault));
        f.env.traversal = Some(ledge(1.4, 3.0));
        assert_eq!(kind(&f), Some(TraversalKind::ClimbUp));
        f.env.traversal = Some(ledge(2.5, 3.0));
        assert_eq!(f.eval(standing, 0.0), Some(LocomotionState::Jump));
    }

    #[test]
    fn half_meter_ledge_climbs_without_a_button() {
        let mut f = Fixture::new();
        f.intent.direction = Vec3::NEG_Z;
        f.env.traversal = Some(ledge(0.5, 3.0));
        let standing = grounded(Stance::Standing, Gait::Run);
        assert_eq!(f.eval(standing, 0.0), None);

        f.memory.obstacle_push = 1.0;
        assert!(matches!(
            f.eval(standing, 0.0),
            Some(LocomotionState::Traversal(TraversalMotion { kind: TraversalKind::HalfMeterClimb, .. }))
        ));
    }

    #[test]
    fn falling_onto_a_ledge_catches_it() {
        let mut f = Fixture::new();
        f.ground.grounded = false;
        f.intent.direction = Vec3::NEG_Z;
        f.env.traversal = Some(ledge(1.2, 3.0));
        assert!(matches!(
            f.eval(LocomotionState::Fall { apex_y: 2.0 }, 0.2),
            Some(LocomotionState::Traversal(TraversalMotion { kind: TraversalKind::LedgeCatch(_), .. }))
        ));
    }

    #[test]
    fn landing_kind_scales_with_fall_height() {
        let mut f = Fixture::new();
        let land = |f: &Fixture, apex_y| f.eval(LocomotionState::Fall { apex_y }, 0.5);
        assert_eq!(land(&f, 0.5), Some(LocomotionState::Landing { kind: LandingKind::Light, moving: false }));
        assert_eq!(land(&f, 2.0), Some(LocomotionState::Landing { kind: LandingKind::Hard, moving: false }));
        f.intent.direction = Vec3::X;
        f.velocity = Vec3::new(3.0, -8.0, 0.0);
        assert_eq!(land(&f, 5.0), Some(LocomotionState::Landing { kind: LandingKind::Heavy, moving: true }));
    }

    #[test]
    fn moving_landing_needs_stick_agreeing_with_momentum() {
        let mut f = Fixture::new();
        f.intent.direction = Vec3::X;
        f.velocity = Vec3::X * 4.0;
        let landing = |f: &Fixture| f.eval(LocomotionState::Fall { apex_y: 0.5 }, 0.5);
        assert_eq!(landing(&f), Some(LocomotionState::Landing { kind: LandingKind::Light, moving: true }));

        // Stick pulling hard against the momentum: stop first.
        f.velocity = Vec3::NEG_X * 4.0;
        assert_eq!(landing(&f), Some(LocomotionState::Landing { kind: LandingKind::Light, moving: false }));
    }
}
