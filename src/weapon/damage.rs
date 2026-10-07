//! Damage per weapon and target type, from `dc1/weapon-damages.bin`.
//! Each game record is (int, float a, float b). The values below are
//! confirmed; reading the int as damage per hit and the floats as falloff
//! start/end distances (`-1` = no falloff) is a guess.

use bevy::prelude::*;

use super::WeaponId;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TargetKind {
    /// The game's `default` row.
    #[default]
    Human,
    Runner,
    Bloater,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageRecord {
    pub damage: f32,
    /// Distances (m) over which damage falls off; `None` = no falloff.
    pub falloff: Option<(f32, f32)>,
}

impl DamageRecord {
    const fn new(damage: f32, a: f32, b: f32) -> Self {
        let falloff = if a < 0.0 { None } else { Some((a, b)) };
        Self { damage, falloff }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct DamageTable {
    /// `None` target = the weapon's default row.
    pub rows: Vec<(WeaponId, Option<TargetKind>, DamageRecord)>,
    /// placeholder (measure): damage multiplier at and beyond the falloff end.
    pub falloff_min_scale: f32,
    /// placeholder (measure): the game's hit-box names include `targHead`.
    pub headshot_scale: f32,
}

impl Default for DamageTable {
    fn default() -> Self {
        use TargetKind::*;
        use WeaponId::*;
        Self {
            rows: vec![
                (HuntingRifle, None, DamageRecord::new(80.0, 150.0, 300.0)),
                (HuntingRifle, Some(Bloater), DamageRecord::new(70.0, -1.0, -1.0)),
                (Pistol, None, DamageRecord::new(34.0, 30.0, 30.0)),
                (Pistol, Some(Runner), DamageRecord::new(30.0, -1.0, -1.0)),
                (Pistol, Some(Bloater), DamageRecord::new(40.0, -1.0, -1.0)),
                // Melee: the game has no `default` melee row, and the melee
                // rows are the same for almost every melee weapon (confirmed),
                // so real per-weapon damage lives in melee move data instead.
                // The default rows are placeholders; the infected rows are the
                // confirmed knife values.
                (Melee, None, DamageRecord::new(35.0, -1.0, -1.0)),
                (Melee, Some(Runner), DamageRecord::new(12.0, -1.0, -1.0)),
                (Melee, Some(Bloater), DamageRecord::new(8.0, -1.0, -1.0)),
                (Unarmed, None, DamageRecord::new(15.0, -1.0, -1.0)),
            ],
            falloff_min_scale: 0.5,
            headshot_scale: 3.0,
        }
    }
}

impl DamageTable {
    pub fn record(&self, weapon: WeaponId, target: TargetKind) -> Option<DamageRecord> {
        let row = |want: Option<TargetKind>| {
            self.rows.iter().find(|(w, t, _)| *w == weapon && *t == want).map(|&(_, _, r)| r)
        };
        row(Some(target)).or_else(|| row(None))
    }

    pub fn damage(&self, weapon: WeaponId, target: TargetKind, distance: f32, headshot: bool) -> f32 {
        let Some(record) = self.record(weapon, target) else { return 0.0 };
        let falloff = match record.falloff {
            None => 1.0,
            Some((start, end)) if end <= start => {
                if distance < start { 1.0 } else { self.falloff_min_scale }
            }
            Some((start, end)) => {
                let t = ((distance - start) / (end - start)).clamp(0.0, 1.0);
                1.0 + (self.falloff_min_scale - 1.0) * t
            }
        };
        let head = if headshot { self.headshot_scale } else { 1.0 };
        record.damage * falloff * head
    }
}

/// Something that can be shot. Hit-tested as a box.
#[derive(Component, Debug, Clone)]
pub struct Target {
    pub kind: TargetKind,
    pub health: f32,
    pub max_health: f32,
    pub half_extents: Vec3,
    /// Hits this close to the top of the box count as headshots.
    pub head_height: f32,
}

impl Target {
    pub fn new(kind: TargetKind, half_extents: Vec3) -> Self {
        // placeholder (measure): NPC health is not known.
        let max_health = match kind {
            TargetKind::Human => 100.0,
            TargetKind::Runner => 80.0,
            TargetKind::Bloater => 500.0,
        };
        Self { kind, health: max_health, max_health, half_extents, head_height: 0.3 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specific_rows_override_the_default() {
        let table = DamageTable::default();
        let rifle = |kind| table.damage(WeaponId::HuntingRifle, kind, 10.0, false);
        assert_eq!(rifle(TargetKind::Human), 80.0);
        assert_eq!(rifle(TargetKind::Runner), 80.0);
        assert_eq!(rifle(TargetKind::Bloater), 70.0);
        assert_eq!(table.damage(WeaponId::Melee, TargetKind::Bloater, 1.0, false), 8.0);
        assert_eq!(table.damage(WeaponId::HuntingRifle, TargetKind::Human, 10.0, true), 240.0);
    }

    #[test]
    fn falloff_scales_between_start_and_end() {
        let table = DamageTable::default();
        let rifle = |d| table.damage(WeaponId::HuntingRifle, TargetKind::Human, d, false);
        assert_eq!(rifle(150.0), 80.0);
        assert_eq!(rifle(225.0), 60.0);
        assert_eq!(rifle(500.0), 40.0);

        // start == end: a step.
        let pistol = |d| table.damage(WeaponId::Pistol, TargetKind::Human, d, false);
        assert_eq!(pistol(29.0), 34.0);
        assert_eq!(pistol(31.0), 17.0);
        // No falloff for the bloater row.
        assert_eq!(table.damage(WeaponId::Pistol, TargetKind::Bloater, 99.0, false), 40.0);
    }
}
