//! Edge traversal: vaults, climbs and ledge catches. The game picks a
//! traversal by obstacle shape, then by height bucket (its anim tables are
//! named like `Vault1mWalk`, `JumpUp0_5mRun`) and by entry gait.

use bevy::prelude::*;

use super::tuning::LocomotionTuning;

/// What the forward probe found, measured from the character's feet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraversalCandidate {
    /// Obstacle top relative to the feet.
    pub height: f32,
    /// Obstacle thickness along the probe direction.
    pub depth: f32,
    /// Point on top of the near edge.
    pub edge: Vec3,
    /// Where to stand after climbing on top.
    pub onto: Vec3,
    /// Where to land after vaulting across.
    pub over: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TraversalKind {
    /// Over a thin obstacle and down the far side.
    Vault,
    /// Low ledge, triggered just by pushing into it.
    HalfMeterClimb,
    /// Mantle onto a higher surface.
    ClimbUp,
    /// Grabbed from the air (the game's knee/waist/chest landings).
    LedgeCatch(LedgeHeight),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LedgeHeight {
    Knee,
    Waist,
    Chest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HeightBucket {
    Half,
    One,
    OneAndHalf,
}

impl HeightBucket {
    pub fn from_height(height: f32) -> Self {
        if height < 0.75 {
            Self::Half
        } else if height < 1.25 {
            Self::One
        } else {
            Self::OneAndHalf
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntryGait {
    Idle,
    Walk,
    Run,
}

impl EntryGait {
    pub fn from_speed(speed: f32, t: &LocomotionTuning) -> Self {
        if speed < 0.5 * t.walk_speed {
            Self::Idle
        } else if speed < 0.5 * (t.walk_speed + t.run_speed) {
            Self::Walk
        } else {
            Self::Run
        }
    }
}

/// Kinematic trajectory, fixed at entry so it is deterministic and animation
/// can be time-warped onto it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraversalMotion {
    pub kind: TraversalKind,
    pub bucket: HeightBucket,
    pub entry: EntryGait,
    pub start: Vec3,
    pub end: Vec3,
    pub apex_y: f32,
    pub duration: f32,
}

fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

impl TraversalMotion {
    pub fn sample(&self, t: f32) -> Vec3 {
        let s = (t / self.duration).clamp(0.0, 1.0);
        match self.kind {
            TraversalKind::Vault => {
                // Eased travel plus a parabolic lift peaking at `apex_y` halfway.
                let mut p = self.start.lerp(self.end, smoothstep(s));
                let base_y = self.start.y + (self.end.y - self.start.y) * s;
                let mid_y = 0.5 * (self.start.y + self.end.y);
                p.y = base_y + 4.0 * s * (1.0 - s) * (self.apex_y - mid_y);
                p
            }
            // Mantle: rise to the ledge first, then pull forward over it.
            _ => {
                let rise = smoothstep(s / 0.6);
                let forward = smoothstep((s - 0.35) / 0.65);
                let mut p = self.start.lerp(self.end, forward);
                p.y = self.start.y + (self.end.y - self.start.y) * rise;
                p
            }
        }
    }
}

pub fn classify_grounded(c: &TraversalCandidate, t: &LocomotionTuning) -> Option<TraversalKind> {
    if c.depth <= t.vault_max_depth {
        (t.vault_min_height..=t.vault_max_height)
            .contains(&c.height)
            .then_some(TraversalKind::Vault)
    } else if c.height <= t.half_meter_max_height {
        Some(TraversalKind::HalfMeterClimb)
    } else if c.height <= t.climb_max_height {
        Some(TraversalKind::ClimbUp)
    } else {
        None
    }
}

pub fn classify_airborne(c: &TraversalCandidate, t: &LocomotionTuning) -> Option<TraversalKind> {
    let standable = c.depth > t.vault_max_depth;
    let in_reach = (t.ledge_catch_min_height..=t.ledge_catch_max_height).contains(&c.height);
    (standable && in_reach).then(|| {
        TraversalKind::LedgeCatch(if c.height < 0.6 {
            LedgeHeight::Knee
        } else if c.height < 1.0 {
            LedgeHeight::Waist
        } else {
            LedgeHeight::Chest
        })
    })
}

pub fn plan(
    kind: TraversalKind,
    c: &TraversalCandidate,
    start: Vec3,
    entry: EntryGait,
    t: &LocomotionTuning,
) -> TraversalMotion {
    let bucket = HeightBucket::from_height(c.height);
    let end = if kind == TraversalKind::Vault { c.over } else { c.onto };
    TraversalMotion {
        kind,
        bucket,
        entry,
        start,
        end,
        apex_y: c.edge.y + t.vault_clearance,
        duration: t.traversal_duration(kind, bucket, entry),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(height: f32, depth: f32) -> TraversalCandidate {
        TraversalCandidate {
            height,
            depth,
            edge: Vec3::new(0.0, height, -1.0),
            onto: Vec3::new(0.0, height, -1.4),
            over: Vec3::new(0.0, 0.0, -1.6),
        }
    }

    #[test]
    fn classifies_by_shape_and_height() {
        let t = LocomotionTuning::default();
        assert_eq!(classify_grounded(&candidate(0.9, 0.3), &t), Some(TraversalKind::Vault));
        assert_eq!(classify_grounded(&candidate(0.5, 3.0), &t), Some(TraversalKind::HalfMeterClimb));
        assert_eq!(classify_grounded(&candidate(1.5, 3.0), &t), Some(TraversalKind::ClimbUp));
        assert_eq!(classify_grounded(&candidate(2.5, 3.0), &t), None);
        assert_eq!(classify_grounded(&candidate(1.6, 0.3), &t), None);
    }

    #[test]
    fn ledge_catch_height_bands() {
        let t = LocomotionTuning::default();
        let kind = |h| classify_airborne(&candidate(h, 3.0), &t);
        assert_eq!(kind(0.45), Some(TraversalKind::LedgeCatch(LedgeHeight::Knee)));
        assert_eq!(kind(0.8), Some(TraversalKind::LedgeCatch(LedgeHeight::Waist)));
        assert_eq!(kind(1.3), Some(TraversalKind::LedgeCatch(LedgeHeight::Chest)));
        assert_eq!(kind(0.1), None);
    }

    #[test]
    fn curves_hit_their_endpoints() {
        let t = LocomotionTuning::default();
        for kind in [TraversalKind::Vault, TraversalKind::ClimbUp] {
            let m = plan(kind, &candidate(0.9, 0.3), Vec3::ZERO, EntryGait::Run, &t);
            assert!(m.sample(0.0).abs_diff_eq(m.start, 1e-5));
            assert!(m.sample(m.duration).abs_diff_eq(m.end, 1e-5));
        }
    }
}
