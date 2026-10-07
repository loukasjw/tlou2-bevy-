//! Weapons: the bolt-action hunting rifle and the pistol with an optional
//! silencer. Shots are hitscan from the camera through the screen centre,
//! so sway and recoil move the view itself.
//! Runs after the camera: Input -> locomotion -> camera -> weapons.

pub mod damage;
pub mod melee;
pub mod systems;
pub mod tuning;

use bevy::prelude::*;

use damage::DamageTable;
use melee::MeleeState;
use tuning::WeaponTable;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WeaponId {
    #[default]
    Unarmed,
    /// confirmed: `rifle-remington-bolt`, bolt action.
    HuntingRifle,
    /// confirmed: `pistol-beretta` (Ellie). The silencer is an upgrade of it.
    Pistol,
    /// A held melee weapon. No attacks yet; its real identity is pending research.
    Melee,
}

impl WeaponId {
    pub fn can_aim(self) -> bool {
        matches!(self, Self::HuntingRifle | Self::Pistol)
    }
}

#[derive(Component, Debug, Default, Clone, Copy)]
#[require(WeaponInput, Arsenal, MeleeState)]
pub struct Loadout {
    pub equipped: WeaponId,
    pub aiming: bool,
}

/// What the shooter wants this frame. Written by player input or AI.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct WeaponInput {
    /// Edge-triggered: true only on the frame the trigger is pulled.
    pub fire: bool,
    pub reload: bool,
    /// Held: steady the rifle while scoped.
    pub hold_breath: bool,
    pub toggle_silencer: bool,
    /// Edge-triggered melee attack (only when not aiming).
    pub melee: bool,
}

/// Fitted to the pistol. A workbench upgrade in the game (`silencer` in
/// `weapon-upgrades`, confirmed), so it is a modifier, not a separate gun.
/// It wears out: the game tracks its health per weapon
/// (`player-get-weapon-silencer-health`, confirmed); the amount is not known.
#[derive(Component, Debug, Clone, Copy)]
pub struct Silencer {
    /// Shots left before it breaks.
    pub health: u32,
}

impl Silencer {
    /// placeholder (measure)
    pub const MAX_HEALTH: u32 = 20;
}

impl Default for Silencer {
    fn default() -> Self {
        Self { health: Self::MAX_HEALTH }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Ammo {
    pub loaded: u32,
    pub reserve: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Breath {
    pub holding: bool,
    /// How long the current breath has been held.
    pub held: f32,
    /// > 0 while recovering from running out of breath.
    pub recovering: f32,
}

/// Per-shooter weapon state.
#[derive(Component, Debug, Clone)]
pub struct Arsenal {
    pub hunting_rifle: Ammo,
    pub pistol: Ammo,
    /// Time until the next shot is allowed (bolt cycle / trigger reset).
    pub cooldown: f32,
    /// Weapon being reloaded and the time left.
    pub reload: Option<(WeaponId, f32)>,
    pub breath: Breath,
    sway_time: f32,
    rng: u32,
}

impl Default for Arsenal {
    fn default() -> Self {
        let table = WeaponTable::default();
        let full = |t: &tuning::WeaponTuning| Ammo { loaded: t.magazine, reserve: t.starting_reserve };
        Self {
            hunting_rifle: full(&table.hunting_rifle),
            pistol: full(&table.pistol),
            cooldown: 0.0,
            reload: None,
            breath: Breath::default(),
            sway_time: 0.0,
            rng: 0x9e37_79b9,
        }
    }
}

impl Arsenal {
    pub fn ammo(&self, weapon: WeaponId) -> Option<Ammo> {
        match weapon {
            WeaponId::Unarmed | WeaponId::Melee => None,
            WeaponId::HuntingRifle => Some(self.hunting_rifle),
            WeaponId::Pistol => Some(self.pistol),
        }
    }

    fn ammo_mut(&mut self, weapon: WeaponId) -> Option<&mut Ammo> {
        match weapon {
            WeaponId::Unarmed | WeaponId::Melee => None,
            WeaponId::HuntingRifle => Some(&mut self.hunting_rifle),
            WeaponId::Pistol => Some(&mut self.pistol),
        }
    }

    /// xorshift32, uniform in `0..1`. Enough for spread and recoil jitter.
    fn random(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1 << 24) as f32
    }
}

/// Testing aid: reloads never use up spare ammo.
#[derive(Resource, Debug, Clone)]
pub struct AmmoSettings {
    pub infinite_reserve: bool,
}

impl Default for AmmoSettings {
    fn default() -> Self {
        Self { infinite_reserve: true }
    }
}

/// A sound that NPCs could react to. Nothing listens yet except the debug rings.
#[derive(Message, Debug, Clone, Copy)]
pub struct Noise {
    pub position: Vec3,
    pub radius: f32,
}

pub struct WeaponPlugin;

impl Plugin for WeaponPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeaponTable>()
            .init_resource::<DamageTable>()
            .init_resource::<AmmoSettings>()
            .init_resource::<systems::ShotEffects>()
            .add_message::<Noise>()
            .add_systems(
                Update,
                (systems::update_weapons, melee::update_melee, systems::update_targets, systems::draw_effects)
                    .chain()
                    .after(crate::camera::update_orbit_camera),
            );
    }
}
