//! Per-weapon handling. Every number is a placeholder (measure) unless
//! tagged: fire rate, magazine, reload, recoil, spread, sway, breath and noise
//! live in DC structs (`weapon-gameplay`, `weapon-upgrades`) that are not
//! decoded yet. answer3.md found them absent from every decoded struct.
//!
//! Also seen in `weapon-gameplay` but not used yet (meanings are guesses):
//! +0x6c, possibly an aim-in time: rifle 0.9, pistol 0.4.
//! +0x70, set only on bolt-action rifles: 0.4.

use bevy::prelude::*;

use super::WeaponId;
use crate::locomotion::state::Stance;

#[derive(Debug, Clone)]
pub struct HoldBreath {
    /// Sway multiplier while the breath is held.
    pub sway_scale: f32,
    /// How long the breath can be held (s).
    pub max_time: f32,
    /// Wobble after running out of breath (s), and how strong it is.
    pub recovery_time: f32,
    pub recovery_sway_scale: f32,
}

#[derive(Debug, Clone)]
pub struct WeaponTuning {
    pub magazine: u32,
    pub starting_reserve: u32,
    /// Minimum time between shots: the bolt cycle for the rifle, trigger
    /// reset for the pistol.
    pub fire_interval: f32,
    pub reload_time: f32,
    /// Spread cone half-angle while aiming still (rad), and the extra added at run speed.
    pub spread: f32,
    pub moving_spread: f32,
    /// Upward camera kick per shot (rad) and the random sideways part.
    pub recoil_pitch: f32,
    pub recoil_yaw: f32,
    /// Aim sway: amplitude (rad) and the speed of the figure-eight (Hz).
    pub sway_amplitude: f32,
    pub sway_frequency: f32,
    pub hold_breath: Option<HoldBreath>,
    /// How far a shot is heard (m), and with the silencer fitted.
    pub noise_radius: f32,
    pub silenced_noise_radius: Option<f32>,
    pub range: f32,
}

#[derive(Resource, Debug, Clone)]
pub struct WeaponTable {
    pub hunting_rifle: WeaponTuning,
    pub pistol: WeaponTuning,
}

impl WeaponTable {
    pub fn get(&self, id: WeaponId) -> Option<&WeaponTuning> {
        match id {
            WeaponId::Unarmed | WeaponId::Melee => None,
            WeaponId::HuntingRifle => Some(&self.hunting_rifle),
            WeaponId::Pistol => Some(&self.pistol),
        }
    }
}

impl Default for WeaponTable {
    fn default() -> Self {
        Self {
            hunting_rifle: WeaponTuning {
                magazine: 4,
                starting_reserve: 8,
                fire_interval: 1.3,
                reload_time: 2.6,
                spread: 0.0,
                moving_spread: 0.02,
                recoil_pitch: 0.07,
                recoil_yaw: 0.02,
                sway_amplitude: 0.012,
                sway_frequency: 0.35,
                hold_breath: Some(HoldBreath {
                    sway_scale: 0.1,
                    max_time: 4.0,
                    recovery_time: 2.0,
                    recovery_sway_scale: 2.5,
                }),
                noise_radius: 60.0,
                silenced_noise_radius: None,
                // guess: `weapon-gameplay` +0x80 = 1500 (read as max range).
                range: 1500.0,
            },
            pistol: WeaponTuning {
                magazine: 8,
                starting_reserve: 24,
                fire_interval: 0.28,
                reload_time: 1.6,
                spread: 0.006,
                moving_spread: 0.03,
                recoil_pitch: 0.025,
                recoil_yaw: 0.01,
                sway_amplitude: 0.006,
                sway_frequency: 0.5,
                hold_breath: None,
                noise_radius: 30.0,
                silenced_noise_radius: Some(4.0),
                // guess: `weapon-gameplay` +0x80 = 60 (read as max range).
                range: 60.0,
            },
        }
    }
}

/// Lower stances steady the aim.
pub fn stance_sway_scale(stance: Stance) -> f32 {
    match stance {
        Stance::Standing => 1.0,
        Stance::Crouching => 0.7,
        Stance::Prone => 0.4,
    }
}
