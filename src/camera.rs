//! Over-the-shoulder orbit camera. Framing (shoulder offset, pivot height,
//! distance, FOV) is picked from a condition table modeled on the game's
//! `camera-zoom-offsets` records in `camera-zoom.bin`, which are selected by
//! conditions such as `weapon-id` and `is-crouched` (confirmed structure).
//! Every number in the table is a placeholder (measure).

use bevy::prelude::*;

use crate::locomotion::LocomotionSet;
use crate::locomotion::sensors::Obstacle;
use crate::locomotion::state::{Locomotion, Stance};
use crate::weapon::{Loadout, WeaponId};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Framing {
    /// Relative to the feet, in camera-yaw space: x = right of the player,
    /// y = pivot height, z = distance behind the pivot.
    pub offset: Vec3,
    pub fov_degrees: f32,
}

impl Framing {
    const fn new(right: f32, height: f32, distance: f32, fov_degrees: f32) -> Self {
        Self { offset: Vec3::new(right, height, distance), fov_degrees }
    }

    fn lerp(self, to: Self, t: f32) -> Self {
        Self {
            offset: self.offset.lerp(to.offset, t),
            fov_degrees: self.fov_degrees + (to.fov_degrees - self.fov_degrees) * t,
        }
    }
}

/// What a table row tests. `None` fields match anything.
#[derive(Clone, Copy, Debug, Default)]
pub struct Condition {
    pub weapon: Option<WeaponId>,
    pub stance: Option<Stance>,
    pub aiming: Option<bool>,
}

#[derive(Clone, Copy, Debug)]
pub struct CameraContext {
    pub weapon: WeaponId,
    pub stance: Stance,
    pub aiming: bool,
}

impl Condition {
    fn matches(&self, ctx: &CameraContext) -> bool {
        self.weapon.is_none_or(|w| w == ctx.weapon)
            && self.stance.is_none_or(|s| s == ctx.stance)
            && self.aiming.is_none_or(|a| a == ctx.aiming)
    }
}

/// Rows are checked in order and the first match wins, so list specific rows first.
#[derive(Resource, Debug, Clone)]
pub struct CameraZoomOffsets {
    pub rows: Vec<(Condition, Framing)>,
    pub default: Framing,
}

impl CameraZoomOffsets {
    pub fn select(&self, ctx: &CameraContext) -> Framing {
        self.rows.iter().find(|(c, _)| c.matches(ctx)).map_or(self.default, |&(_, f)| f)
    }
}

impl Default for CameraZoomOffsets {
    fn default() -> Self {
        use Stance::*;
        let row = |weapon, stance, aiming, framing| (Condition { weapon, stance, aiming }, framing);
        let rifle = Some(WeaponId::HuntingRifle);
        Self {
            rows: vec![
                // Rifle aim is the scope: the camera moves to the eye and zooms.
                // guess: scope FOV 35 from `weapon-gameplay` +0xa8 (about 3.2x).
                row(rifle, Some(Prone), Some(true), Framing::new(0.0, 0.45, 0.0, 35.0)),
                row(rifle, Some(Crouching), Some(true), Framing::new(0.0, 1.0, 0.0, 35.0)),
                row(rifle, None, Some(true), Framing::new(0.0, 1.6, 0.0, 35.0)),
                // Other aiming: tight over the right shoulder.
                // guess: aim FOV 60 for the pistol from `weapon-gameplay` +0xa0.
                row(None, Some(Prone), Some(true), Framing::new(0.35, 0.5, 1.3, 60.0)),
                row(None, Some(Crouching), Some(true), Framing::new(0.5, 1.05, 1.5, 60.0)),
                row(None, None, Some(true), Framing::new(0.5, 1.55, 1.5, 60.0)),
                // Exploring.
                row(None, Some(Prone), None, Framing::new(0.35, 0.6, 2.2, 60.0)),
                row(None, Some(Crouching), None, Framing::new(0.55, 1.1, 2.6, 60.0)),
            ],
            default: Framing::new(0.6, 1.6, 3.0, 60.0),
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct CameraTuning {
    pub min_pitch: f32,
    pub max_pitch: f32,
    /// Exponential rate (1/s) at which framing moves to a new table row.
    pub framing_rate: f32,
    /// Faster rate for aiming in, so the zoom feels responsive.
    pub aim_in_rate: f32,
    /// Gap kept between the camera and a wall it is pulled in front of.
    pub wall_margin: f32,
}

impl Default for CameraTuning {
    fn default() -> Self {
        Self {
            min_pitch: -70f32.to_radians(),
            max_pitch: 55f32.to_radians(),
            framing_rate: 6.0,
            aim_in_rate: 14.0,
            wall_margin: 0.2,
        }
    }
}

#[derive(Component, Debug)]
#[require(Camera3d)]
pub struct OrbitCamera {
    pub target: Entity,
    pub yaw: f32,
    pub pitch: f32,
    /// Look input for this frame in radians (x = yaw, y = pitch), written by
    /// game input and consumed by the camera.
    pub look: Vec2,
    /// Aim sway (x = yaw, y = pitch), added on top of the view without
    /// accumulating. Written by the weapon systems.
    pub sway: Vec2,
    framing: Option<Framing>,
}

impl OrbitCamera {
    pub fn new(target: Entity) -> Self {
        Self { target, yaw: 0.0, pitch: -0.25, look: Vec2::ZERO, sway: Vec2::ZERO, framing: None }
    }

    /// Horizontal forward and right of the view, for camera-relative input.
    /// Includes look input not yet applied, so movement matches the view
    /// the player is steering toward this frame.
    pub fn ground_axes(&self) -> (Vec3, Vec3) {
        let yaw = Quat::from_rotation_y(self.yaw + self.look.x);
        (yaw * Vec3::NEG_Z, yaw * Vec3::X)
    }
}

pub struct CameraRigPlugin;

impl Plugin for CameraRigPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraZoomOffsets>()
            .init_resource::<CameraTuning>()
            .add_systems(Update, update_orbit_camera.after(LocomotionSet::Integrate));
    }
}

pub fn update_orbit_camera(
    time: Res<Time>,
    table: Res<CameraZoomOffsets>,
    tuning: Res<CameraTuning>,
    targets: Query<(&Transform, &Locomotion, Option<&Loadout>), Without<OrbitCamera>>,
    obstacles: Query<(&Transform, &Obstacle), Without<OrbitCamera>>,
    mut cameras: Query<(&mut OrbitCamera, &mut Transform, &mut Projection)>,
) {
    let dt = time.delta_secs();
    for (mut cam, mut transform, mut projection) in &mut cameras {
        let Ok((target, loco, loadout)) = targets.get(cam.target) else { continue };

        cam.yaw += cam.look.x;
        cam.pitch = (cam.pitch + cam.look.y).clamp(tuning.min_pitch, tuning.max_pitch);
        cam.look = Vec2::ZERO;

        let loadout = loadout.copied().unwrap_or_default();
        let ctx = CameraContext { weapon: loadout.equipped, stance: loco.state.stance(), aiming: loadout.aiming };
        let goal = table.select(&ctx);
        let rate = if ctx.aiming { tuning.aim_in_rate } else { tuning.framing_rate };
        let framing = match cam.framing {
            Some(current) => current.lerp(goal, 1.0 - (-rate * dt).exp()),
            None => goal,
        };
        cam.framing = Some(framing);

        let rotation = Quat::from_euler(EulerRot::YXZ, cam.yaw + cam.sway.x, cam.pitch + cam.sway.y, 0.0);
        let pivot = target.translation + Vec3::Y * framing.offset.y;
        // Pitch turns about X, so the shoulder offset stays horizontal.
        let wanted = pivot + rotation * Vec3::new(framing.offset.x, 0.0, framing.offset.z);

        // Pull in front of any box between the player and the camera.
        let to_camera = wanted - pivot;
        let length = to_camera.length();
        let dir = to_camera / length.max(1e-4);
        let hit = obstacles
            .iter()
            .filter_map(|(t, o)| ray_box(pivot, dir, t.translation - o.half_extents, t.translation + o.half_extents))
            .fold(length, f32::min);
        let distance = if hit < length { (hit - tuning.wall_margin).max(0.0) } else { length };

        transform.translation = pivot + dir * distance;
        transform.rotation = rotation;
        if let Projection::Perspective(perspective) = projection.as_mut() {
            perspective.fov = framing.fov_degrees.to_radians();
        }
    }
}

/// Slab test: distance along `dir` to where the ray enters the box. Rays that
/// start inside a box ignore it, so a low roof over the player doesn't
/// collapse the camera onto the pivot.
pub fn ray_box(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let inv = dir.recip();
    let t1 = (min - origin) * inv;
    let t2 = (max - origin) * inv;
    let enter = t1.min(t2).max_element();
    let exit = t1.max(t2).min_element();
    (enter >= 0.0 && exit >= enter).then_some(enter)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(weapon: WeaponId, stance: Stance, aiming: bool) -> CameraContext {
        CameraContext { weapon, stance, aiming }
    }

    #[test]
    fn rows_match_most_specific_first() {
        let table = CameraZoomOffsets::default();
        let explore = table.select(&ctx(WeaponId::Unarmed, Stance::Standing, false));
        assert_eq!(explore, table.default);

        let rifle = table.select(&ctx(WeaponId::HuntingRifle, Stance::Standing, true));
        let pistol = table.select(&ctx(WeaponId::Pistol, Stance::Standing, true));
        assert!(rifle.offset.z < pistol.offset.z, "rifle aim should sit closer than pistol");
        assert!(rifle.fov_degrees < pistol.fov_degrees, "rifle scope should zoom");
        assert!(pistol.offset.z < explore.offset.z);

        let crouched = table.select(&ctx(WeaponId::Pistol, Stance::Crouching, true));
        assert!(crouched.offset.y < pistol.offset.y);
    }

    #[test]
    fn ray_box_hits_in_front_and_ignores_behind_and_inside() {
        let (min, max) = (Vec3::new(-1.0, 0.0, 2.0), Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(ray_box(Vec3::Y, Vec3::Z, min, max), Some(2.0));
        assert_eq!(ray_box(Vec3::Y, Vec3::NEG_Z, min, max), None);
        assert_eq!(ray_box(Vec3::new(0.0, 1.0, 2.5), Vec3::Z, min, max), None);
    }
}
