use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::prelude::*;

use super::damage::{DamageTable, Target, TargetKind};
use super::tuning::{stance_sway_scale, HoldBreath, WeaponTable};
use super::{AmmoSettings, Arsenal, Breath, Loadout, Noise, Silencer, WeaponInput};
use crate::camera::{ray_box, ray_capsule, OrbitCamera};
use crate::locomotion::components::KinematicBody;
use crate::locomotion::sensors::Obstacle;
use crate::locomotion::state::{Locomotion, LocomotionState};
use crate::locomotion::tuning::LocomotionTuning;

const TRACER_TIME: f32 = 0.12;
const RING_TIME: f32 = 0.6;
const DOWNED_TIME: f32 = 3.0;
const FLASH_TIME: f32 = 0.1;

#[derive(Debug, Clone)]
pub struct HitReport {
    pub kind: TargetKind,
    pub headshot: bool,
    pub damage: f32,
    pub distance: f32,
    pub health_left: f32,
}

/// Debug visuals for shots, plus the last hit for the HUD.
#[derive(Resource, Debug, Default)]
pub struct ShotEffects {
    tracers: Vec<(Vec3, Vec3, f32)>,
    rings: Vec<(Noise, f32)>,
    pub last_hit: Option<HitReport>,
}

/// Brief white flash on hit; remembers the target's own colour.
#[derive(Component, Debug, Clone)]
pub struct HitFlash {
    pub base: Color,
    timer: f32,
}

impl HitFlash {
    pub fn new(base: Color) -> Self {
        Self { base, timer: 0.0 }
    }

    pub fn flash(&mut self) {
        self.timer = FLASH_TIME;
    }
}

/// A target that was shot down; it stands back up after a moment.
#[derive(Component, Debug)]
pub struct Downed(f32);

/// Returns the sway multiplier from holding (or running out of) breath.
fn update_breath(breath: &mut Breath, hold: Option<&HoldBreath>, wants_hold: bool, dt: f32) -> f32 {
    let Some(hold) = hold else {
        *breath = Breath::default();
        return 1.0;
    };
    if breath.recovering > 0.0 {
        breath.recovering = (breath.recovering - dt).max(0.0);
        breath.holding = false;
        return hold.recovery_sway_scale;
    }
    breath.holding = wants_hold;
    if !wants_hold {
        // Breathing normally refills the lungs at the same rate.
        breath.held = (breath.held - dt).max(0.0);
        return 1.0;
    }
    breath.held += dt;
    if breath.held >= hold.max_time {
        breath.held = 0.0;
        breath.recovering = hold.recovery_time;
        breath.holding = false;
    }
    hold.sway_scale
}

/// Random direction inside a cone, uniform over its area.
fn spread_direction(forward: Vec3, half_angle: f32, arsenal: &mut Arsenal) -> Vec3 {
    if half_angle <= 0.0 {
        return forward;
    }
    let (a, b) = forward.any_orthonormal_pair();
    let angle = half_angle * arsenal.random().sqrt();
    let phi = TAU * arsenal.random();
    (forward * angle.cos() + (a * phi.cos() + b * phi.sin()) * angle.sin()).normalize()
}

#[allow(clippy::too_many_arguments)]
pub fn update_weapons(
    time: Res<Time>,
    table: Res<WeaponTable>,
    damage: Res<DamageTable>,
    mut effects: ResMut<ShotEffects>,
    settings: Res<AmmoSettings>,
    mut noises: MessageWriter<Noise>,
    mut commands: Commands,
    mut shooters: Query<(
        Entity,
        &Loadout,
        &WeaponInput,
        &mut Arsenal,
        &Locomotion,
        &KinematicBody,
        &LocomotionTuning,
        &Transform,
        Option<&mut Silencer>,
    )>,
    mut cameras: Query<(&mut OrbitCamera, &Transform)>,
    obstacles: Query<(&Transform, &Obstacle)>,
    mut targets: Query<(Entity, &Transform, &mut Target, &mut HitFlash)>,
) {
    let dt = time.delta_secs();
    for (entity, loadout, input, mut arsenal, loco, body, loco_tuning, transform, mut silencer) in &mut shooters {
        if input.toggle_silencer {
            if silencer.is_some() {
                commands.entity(entity).remove::<Silencer>();
            } else {
                commands.entity(entity).insert(Silencer::default());
            }
        }
        let Some((mut cam, cam_transform)) = cameras.iter_mut().find(|(c, _)| c.target == entity) else {
            continue;
        };
        let weapon = loadout.equipped;
        let Some(tuning) = table.get(weapon) else {
            arsenal.reload = None;
            arsenal.breath = Breath::default();
            cam.sway = Vec2::ZERO;
            continue;
        };

        arsenal.cooldown = (arsenal.cooldown - dt).max(0.0);
        arsenal.reload = match arsenal.reload {
            // Switching weapons cancels a reload.
            Some((w, _)) if w != weapon => None,
            Some((w, left)) if left > dt => Some((w, left - dt)),
            Some(_) => {
                let ammo = arsenal.ammo_mut(weapon).expect("armed");
                let missing = tuning.magazine - ammo.loaded;
                if settings.infinite_reserve {
                    ammo.loaded += missing;
                } else {
                    let moved = missing.min(ammo.reserve);
                    ammo.loaded += moved;
                    ammo.reserve -= moved;
                }
                None
            }
            None => None,
        };
        let ammo = arsenal.ammo(weapon).expect("armed");
        let dry_fire = input.fire && loadout.aiming && ammo.loaded == 0;
        if (input.reload || dry_fire)
            && arsenal.reload.is_none()
            && ammo.loaded < tuning.magazine
            && (ammo.reserve > 0 || settings.infinite_reserve)
        {
            arsenal.reload = Some((weapon, tuning.reload_time));
        }

        // Sway: a slow figure-eight, calmer when low or holding breath, worse on the move.
        let breath_scale =
            update_breath(&mut arsenal.breath, tuning.hold_breath.as_ref(), input.hold_breath && loadout.aiming, dt);
        let moving = (body.velocity.xz().length() / loco_tuning.run_speed).min(1.0);
        let sway = if loadout.aiming {
            arsenal.sway_time += dt;
            let w = TAU * tuning.sway_frequency * arsenal.sway_time;
            let amplitude =
                tuning.sway_amplitude * stance_sway_scale(loco.state.stance()) * (1.0 + 2.0 * moving) * breath_scale;
            Vec2::new(w.sin(), (2.0 * w).sin() * 0.5) * amplitude
        } else {
            Vec2::ZERO
        };
        let sway_follow = 1.0 - (-10.0 * dt).exp();
        cam.sway = cam.sway.lerp(sway, sway_follow);

        let steady = matches!(
            loco.state,
            LocomotionState::Grounded { .. } | LocomotionState::Landing { .. } | LocomotionState::ChangingStance { .. }
        );
        if !(input.fire
            && loadout.aiming
            && steady
            && arsenal.cooldown <= 0.0
            && arsenal.reload.is_none()
            && ammo.loaded > 0)
        {
            continue;
        }

        arsenal.ammo_mut(weapon).expect("armed").loaded -= 1;
        arsenal.cooldown = tuning.fire_interval;

        let origin = cam_transform.translation;
        let spread = tuning.spread + tuning.moving_spread * moving;
        let dir = spread_direction(cam_transform.forward().as_vec3(), spread, &mut arsenal);

        let box_hit = |center: Vec3, half: Vec3| ray_box(origin, dir, center - half, center + half);
        let mut nearest = (tuning.range, None);
        for (t, o) in &obstacles {
            if let Some(d) = box_hit(t.translation, o.half_extents).filter(|&d| d < nearest.0) {
                nearest = (d, None);
            }
        }
        for (e, t, target, _) in &targets {
            if target.health <= 0.0 {
                continue;
            }
            let pill = ray_capsule(origin, dir, t.translation, target.half_extents.x, target.half_extents.y);
            if let Some(d) = pill.filter(|&d| d < nearest.0) {
                nearest = (d, Some(e));
            }
        }
        let hit_point = origin + dir * nearest.0;
        let muzzle = transform.translation + Vec3::Y * loco_tuning.height(loco.state.stance()) * 0.8;

        if let Some((_, t, mut target, mut flash)) = nearest.1.and_then(|e| targets.get_mut(e).ok()) {
            let top = t.translation.y + target.half_extents.y;
            let headshot = hit_point.y >= top - target.head_height;
            let distance = muzzle.distance(hit_point);
            let amount = damage.damage(weapon, target.kind, distance, headshot);
            target.health = (target.health - amount).max(0.0);
            flash.flash();
            effects.last_hit = Some(HitReport {
                kind: target.kind,
                headshot,
                damage: amount,
                distance,
                health_left: target.health,
            });
        }
        effects.tracers.push((muzzle, hit_point, TRACER_TIME));

        let radius = match (tuning.silenced_noise_radius, silencer.as_deref_mut()) {
            (Some(quiet), Some(fitted)) => {
                // Each shot wears the silencer; it falls apart when spent.
                fitted.health = fitted.health.saturating_sub(1);
                if fitted.health == 0 {
                    commands.entity(entity).remove::<Silencer>();
                }
                quiet
            }
            _ => tuning.noise_radius,
        };
        noises.write(Noise { position: muzzle, radius });

        cam.look.y += tuning.recoil_pitch;
        cam.look.x += (arsenal.random() * 2.0 - 1.0) * tuning.recoil_yaw;
    }
}

/// Hit flashes, knock-downs and respawns.
pub fn update_targets(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut targets: Query<(
        Entity,
        &mut Target,
        &mut HitFlash,
        &MeshMaterial3d<StandardMaterial>,
        &mut Visibility,
        Option<&mut Downed>,
    )>,
) {
    let dt = time.delta_secs();
    for (entity, mut target, mut flash, material, mut visibility, downed) in &mut targets {
        match downed {
            Some(mut downed) => {
                downed.0 -= dt;
                if downed.0 <= 0.0 {
                    target.health = target.max_health;
                    *visibility = Visibility::Inherited;
                    commands.entity(entity).remove::<Downed>();
                }
            }
            None if target.health <= 0.0 => {
                *visibility = Visibility::Hidden;
                commands.entity(entity).insert(Downed(DOWNED_TIME));
            }
            None => {}
        }

        if flash.timer > 0.0 {
            flash.timer -= dt;
            let color = if flash.timer > 0.0 { Color::WHITE } else { flash.base };
            if let Some(mut m) = materials.get_mut(&material.0) {
                m.base_color = color;
            }
        }
    }
}

pub fn draw_effects(
    time: Res<Time>,
    mut effects: ResMut<ShotEffects>,
    mut noises: MessageReader<Noise>,
    mut gizmos: Gizmos,
) {
    let dt = time.delta_secs();
    effects.rings.extend(noises.read().map(|n| (*n, RING_TIME)));

    effects.tracers.retain_mut(|(from, to, ttl)| {
        gizmos.line(*from, *to, Color::srgb(1.0, 0.85, 0.3));
        gizmos.sphere(Isometry3d::from_translation(*to), 0.06, Color::srgb(1.0, 0.4, 0.2));
        *ttl -= dt;
        *ttl > 0.0
    });
    // A flat ring that grows to the hearing radius.
    effects.rings.retain_mut(|(noise, ttl)| {
        let t = 1.0 - *ttl / RING_TIME;
        let ground = Vec3::new(noise.position.x, 0.05, noise.position.z);
        let flat = Isometry3d::new(ground, Quat::from_rotation_x(FRAC_PI_2));
        gizmos.circle(flat, noise.radius * t.max(0.05), Color::srgba(0.4, 0.8, 1.0, 1.0 - t));
        *ttl -= dt;
        *ttl > 0.0
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rifle_breath() -> HoldBreath {
        WeaponTable::default().hunting_rifle.hold_breath.unwrap()
    }

    #[test]
    fn holding_breath_steadies_then_runs_out() {
        let hold = rifle_breath();
        let mut breath = Breath::default();
        assert_eq!(update_breath(&mut breath, Some(&hold), true, 0.1), hold.sway_scale);
        assert!(breath.holding);

        // Hold past the limit: forced recovery with extra sway, even while still holding.
        let mut scale = 0.0;
        for _ in 0..50 {
            scale = update_breath(&mut breath, Some(&hold), true, 0.1);
        }
        assert_eq!(scale, hold.recovery_sway_scale);
        assert!(!breath.holding);
    }

    #[test]
    fn spread_stays_inside_the_cone() {
        let mut arsenal = Arsenal::default();
        for _ in 0..200 {
            let dir = spread_direction(Vec3::NEG_Z, 0.05, &mut arsenal);
            assert!(dir.angle_between(Vec3::NEG_Z) <= 0.05 + 1e-4);
        }
        assert_eq!(spread_direction(Vec3::NEG_Z, 0.0, &mut arsenal), Vec3::NEG_Z);
    }
}
