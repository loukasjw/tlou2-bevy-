//! Melee swings. Each swing is wind-up -> active -> recovery. While active,
//! a sweep in front of the player damages each target once (the game runs a
//! per-frame weapon sweep, `CharacterMeleeWeaponAdjust::DamageSweep::
//! HandleDamageSweeps`, confirmed by name). Pressing again before the
//! recovery ends chains the next swing of the combo.
//!
//! Every timing, reach and arc here is a placeholder (measure): research found
//! no melee timing data (answer3.md). Damage comes from `DamageTable`.

use bevy::prelude::*;

use super::damage::{DamageTable, Target};
use super::systems::{HitFlash, HitReport, ShotEffects};
use super::{Loadout, WeaponId, WeaponInput};
use crate::locomotion::state::{Locomotion, LocomotionState};
use crate::locomotion::tuning::LocomotionTuning;

/// placeholder (measure): how far beyond reach a swing will turn to a target.
const TARGET_SNAP_DISTANCE: f32 = 1.5;

#[derive(Debug, Clone, PartialEq)]
pub struct MeleeTuning {
    pub windup: f32,
    pub active: f32,
    pub recovery: f32,
    /// How far in front of the player the sweep reaches (m).
    pub reach: f32,
    /// Half-angle of the sweep around the swing direction (rad).
    pub half_arc: f32,
    /// Swings in a full combo; the last one is the finisher.
    pub combo_length: u32,
    pub finisher_scale: f32,
}

impl MeleeTuning {
    /// The melee weapon swings fast and short; fists (also used while holding
    /// a gun without aiming) are a little slower.
    pub fn for_weapon(weapon: WeaponId) -> Self {
        match weapon {
            WeaponId::Melee => Self {
                windup: 0.12,
                active: 0.1,
                recovery: 0.3,
                reach: 1.3,
                half_arc: 55f32.to_radians(),
                combo_length: 3,
                finisher_scale: 1.5,
            },
            _ => Self {
                windup: 0.15,
                active: 0.1,
                recovery: 0.35,
                reach: 1.1,
                half_arc: 45f32.to_radians(),
                combo_length: 3,
                finisher_scale: 1.5,
            },
        }
    }

    /// Damage table weapon: guns without aiming hit like fists.
    pub fn damage_weapon(weapon: WeaponId) -> WeaponId {
        if weapon == WeaponId::Melee { WeaponId::Melee } else { WeaponId::Unarmed }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwingPhase {
    Windup,
    Active,
    Recovery,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Swing {
    pub phase: SwingPhase,
    /// Time in the current phase.
    pub time: f32,
    /// 0-based position in the combo.
    pub combo: u32,
    pub direction: Vec3,
}

impl Swing {
    pub fn is_finisher(&self, t: &MeleeTuning) -> bool {
        self.combo + 1 >= t.combo_length
    }
}

#[derive(Component, Debug, Default, Clone)]
pub struct MeleeState {
    pub swing: Option<Swing>,
    /// Attack pressed during a swing: chain into the next one.
    queued: bool,
    /// Targets already hit by the current swing.
    hit: Vec<Entity>,
}

impl MeleeState {
    pub fn swinging(&self) -> bool {
        self.swing.is_some()
    }

    /// Advances the swing timeline by `dt`. `direction` is used when a new
    /// swing starts. Pure apart from `self`, so it can be unit-tested.
    pub fn advance(&mut self, t: &MeleeTuning, pressed: bool, direction: Vec3, dt: f32) {
        let start = |combo| Swing { phase: SwingPhase::Windup, time: 0.0, combo, direction };
        let Some(mut swing) = self.swing else {
            if pressed {
                self.swing = Some(start(0));
                self.hit.clear();
            }
            return;
        };
        self.queued |= pressed;
        swing.time += dt;
        let (length, next) = match swing.phase {
            SwingPhase::Windup => (t.windup, Some(SwingPhase::Active)),
            SwingPhase::Active => (t.active, Some(SwingPhase::Recovery)),
            SwingPhase::Recovery => (t.recovery, None),
        };
        // Recovery is the combo window: a queued press chains right away.
        if swing.phase == SwingPhase::Recovery && self.queued && !swing.is_finisher(t) {
            self.swing = Some(start(swing.combo + 1));
            self.queued = false;
            self.hit.clear();
            return;
        }
        if swing.time >= length {
            swing.time -= length;
            match next {
                Some(phase) => swing.phase = phase,
                None => {
                    self.swing = None;
                    self.queued = false;
                    return;
                }
            }
        }
        self.swing = Some(swing);
    }
}

pub fn update_melee(
    time: Res<Time>,
    damage: Res<DamageTable>,
    mut effects: ResMut<ShotEffects>,
    mut attackers: Query<(&Loadout, &WeaponInput, &mut MeleeState, &Locomotion, &LocomotionTuning, &Transform)>,
    mut targets: Query<(Entity, &Transform, &mut Target, &mut HitFlash)>,
) {
    let dt = time.delta_secs();
    for (loadout, input, mut melee, loco, loco_tuning, transform) in &mut attackers {
        let t = MeleeTuning::for_weapon(loadout.equipped);
        // A dodge cancels the swing.
        if matches!(loco.state, LocomotionState::Evade { .. }) {
            *melee = MeleeState::default();
            continue;
        }
        let can_start = !loadout.aiming && matches!(loco.state, LocomotionState::Grounded { .. });
        let facing = transform.forward().as_vec3().with_y(0.0).normalize_or_zero();
        // Like the game's `FindMeleeTargets`: swing at the nearest target
        // roughly in front, if one is close enough.
        let aim = targets
            .iter()
            .filter(|(_, _, target, _)| target.health > 0.0)
            .filter_map(|(_, t2, target, _)| {
                let to = (t2.translation - transform.translation).with_y(0.0);
                let close = to.length() <= t.reach + target.half_extents.x + TARGET_SNAP_DISTANCE;
                (close && facing.dot(to.normalize_or_zero()) > 0.0).then_some(to)
            })
            .min_by(|a, b| a.length().total_cmp(&b.length()))
            .map_or(facing, |to| to.normalize_or_zero());
        melee.advance(&t, input.melee && can_start, aim, dt);

        let Some(swing) = melee.swing.filter(|s| s.phase == SwingPhase::Active) else { continue };
        let chest = transform.translation + Vec3::Y * loco_tuning.height(loco.state.stance()) * 0.6;
        for (entity, target_transform, mut target, mut flash) in &mut targets {
            // The sweep lasts the whole active window; each target is hit once.
            if target.health <= 0.0 || melee.hit.contains(&entity) {
                continue;
            }
            let center = target_transform.translation;
            let to = (center - chest).with_y(0.0);
            let distance = to.length();
            let in_reach = distance <= t.reach + target.half_extents.x;
            let in_arc = distance < 1e-3 || swing.direction.angle_between(to / distance) <= t.half_arc;
            let level = (center.y - chest.y).abs() <= target.half_extents.y + 0.5;
            if !(in_reach && in_arc && level) {
                continue;
            }
            melee.hit.push(entity);
            let weapon = MeleeTuning::damage_weapon(loadout.equipped);
            let scale = if swing.is_finisher(&t) { t.finisher_scale } else { 1.0 };
            let amount = damage.damage(weapon, target.kind, distance, false) * scale;
            target.health = (target.health - amount).max(0.0);
            flash.flash();
            effects.last_hit = Some(HitReport {
                kind: target.kind,
                headshot: false,
                damage: amount,
                distance,
                health_left: target.health,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(state: &mut MeleeState, t: &MeleeTuning, presses: &[f32], until: f32) -> Vec<u32> {
        // Step at 60 Hz, pressing at the listed times; record each swing start.
        let dt = 1.0 / 60.0;
        let mut started = Vec::new();
        let mut last = None;
        let mut now = 0.0;
        while now < until {
            let pressed = presses.iter().any(|&p| p >= now && p < now + dt);
            state.advance(t, pressed, Vec3::NEG_Z, dt);
            let current = state.swing.map(|s| s.combo);
            if current.is_some() && (last.is_none() || current != last) {
                started.push(current.unwrap());
            }
            last = current;
            now += dt;
        }
        started
    }

    #[test]
    fn single_press_swings_once_then_ends() {
        let t = MeleeTuning::for_weapon(WeaponId::Melee);
        let mut state = MeleeState::default();
        assert_eq!(run(&mut state, &t, &[0.0], 2.0), vec![0]);
        assert!(!state.swinging());
    }

    #[test]
    fn presses_during_a_swing_chain_up_to_the_finisher() {
        let t = MeleeTuning::for_weapon(WeaponId::Melee);
        let mut state = MeleeState::default();
        let mashing: Vec<f32> = (0..9).map(|i| i as f32 * 0.1).collect();
        assert_eq!(run(&mut state, &t, &mashing, 1.4), vec![0, 1, 2]);
    }

    #[test]
    fn swing_goes_through_every_phase() {
        let t = MeleeTuning::for_weapon(WeaponId::Melee);
        let mut state = MeleeState::default();
        state.advance(&t, true, Vec3::NEG_Z, 0.0);
        let mut phases = vec![];
        for _ in 0..60 {
            state.advance(&t, false, Vec3::NEG_Z, 1.0 / 60.0);
            if let Some(s) = state.swing.filter(|s| phases.last() != Some(&s.phase)) {
                phases.push(s.phase);
            }
        }
        assert_eq!(phases, vec![SwingPhase::Windup, SwingPhase::Active, SwingPhase::Recovery]);
    }
}
