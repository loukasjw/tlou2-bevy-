//! A rough, original humanoid rig: 17 joints, capsule limbs, no skinning.
//!
//! Each joint is an entity whose local rotation comes from a [`Pose`]. Anything can write a pose
//! (the procedural animator below today, clip playback later) and `apply_pose` moves the joints.
//! The skeleton is this project's own layout, sized for a 1.8 m stand with the feet at the rig root.
//! +X is the character's right, -Z is forward (Bevy convention, same as the locomotion code).

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::prelude::*;

use crate::locomotion::components::{GroundSensor, KinematicBody};
use crate::locomotion::state::{Gait, Locomotion, LocomotionState, Stance};
use crate::locomotion::tuning::LocomotionTuning;
use crate::weapon::Loadout;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Joint {
    Pelvis,
    Spine,
    Chest,
    Neck,
    Head,
    ShoulderL,
    ElbowL,
    HandL,
    ShoulderR,
    ElbowR,
    HandR,
    HipL,
    KneeL,
    FootL,
    HipR,
    KneeR,
    FootR,
}

pub const JOINT_COUNT: usize = 17;

pub const JOINTS: [Joint; JOINT_COUNT] = [
    Joint::Pelvis,
    Joint::Spine,
    Joint::Chest,
    Joint::Neck,
    Joint::Head,
    Joint::ShoulderL,
    Joint::ElbowL,
    Joint::HandL,
    Joint::ShoulderR,
    Joint::ElbowR,
    Joint::HandR,
    Joint::HipL,
    Joint::KneeL,
    Joint::FootL,
    Joint::HipR,
    Joint::KneeR,
    Joint::FootR,
];

impl Joint {
    pub fn index(self) -> usize {
        self as usize
    }

    /// Parent joint; the pelvis hangs from the rig root.
    pub fn parent(self) -> Option<Joint> {
        use Joint::*;
        match self {
            Pelvis => None,
            Spine | HipL | HipR => Some(Pelvis),
            Chest => Some(Spine),
            Neck | ShoulderL | ShoulderR => Some(Chest),
            Head => Some(Neck),
            ElbowL => Some(ShoulderL),
            HandL => Some(ElbowL),
            ElbowR => Some(ShoulderR),
            HandR => Some(ElbowR),
            KneeL => Some(HipL),
            FootL => Some(KneeL),
            KneeR => Some(HipR),
            FootR => Some(KneeR),
        }
    }

    /// Rest position relative to the parent (the pelvis: relative to the rig root, at the feet).
    pub fn rest_offset(self) -> Vec3 {
        use Joint::*;
        match self {
            Pelvis => Vec3::new(0.0, 0.95, 0.0),
            Spine => Vec3::new(0.0, 0.12, 0.0),
            Chest => Vec3::new(0.0, 0.22, 0.0),
            Neck => Vec3::new(0.0, 0.24, 0.0),
            Head => Vec3::new(0.0, 0.10, 0.0),
            ShoulderL => Vec3::new(-0.20, 0.16, 0.0),
            ShoulderR => Vec3::new(0.20, 0.16, 0.0),
            ElbowL | ElbowR => Vec3::new(0.0, -0.30, 0.0),
            HandL | HandR => Vec3::new(0.0, -0.28, 0.0),
            HipL => Vec3::new(-0.10, -0.05, 0.0),
            HipR => Vec3::new(0.10, -0.05, 0.0),
            KneeL | KneeR => Vec3::new(0.0, -0.43, 0.0),
            FootL | FootR => Vec3::new(0.0, -0.40, 0.0),
        }
    }

    /// Capsule drawn from this joint to a point `end` away (joint space), and its radius.
    fn bone(self) -> Option<(Vec3, f32)> {
        use Joint::*;
        match self {
            Pelvis => Some((Vec3::new(0.0, 0.12, 0.0), 0.12)),
            Spine => Some((Vec3::new(0.0, 0.22, 0.0), 0.14)),
            Chest => Some((Vec3::new(0.0, 0.24, 0.0), 0.16)),
            Neck => Some((Vec3::new(0.0, 0.10, 0.0), 0.04)),
            Head => Some((Vec3::new(0.0, 0.20, 0.0), 0.09)),
            ShoulderL | ShoulderR => Some((Vec3::new(0.0, -0.30, 0.0), 0.045)),
            ElbowL | ElbowR => Some((Vec3::new(0.0, -0.28, 0.0), 0.04)),
            HipL | HipR => Some((Vec3::new(0.0, -0.43, 0.0), 0.07)),
            KneeL | KneeR => Some((Vec3::new(0.0, -0.40, 0.0), 0.055)),
            HandL | HandR | FootL | FootR => None,
        }
    }
}

/// Handles to the joint entities, indexed by [`Joint::index`]. Lives on the rig root.
#[derive(Component)]
pub struct Rig {
    pub joints: [Entity; JOINT_COUNT],
}

/// Local rotation of every joint (applied on top of the identity rest orientation) plus a whole-body
/// offset and rotation for the rig root. Blended toward a target by the animator.
#[derive(Component, Clone, Copy, Debug)]
pub struct Pose {
    pub rot: [Quat; JOINT_COUNT],
    /// Added to the pelvis rest position (crouching lowers it).
    pub pelvis_offset: Vec3,
    /// Rotation of the whole rig about the feet (prone lies it down).
    pub root_rot: Quat,
    pub root_offset: Vec3,
}

impl Default for Pose {
    fn default() -> Self {
        Self::REST
    }
}

impl Pose {
    pub const REST: Pose = Pose {
        rot: [Quat::IDENTITY; JOINT_COUNT],
        pelvis_offset: Vec3::ZERO,
        root_rot: Quat::IDENTITY,
        root_offset: Vec3::ZERO,
    };

    pub fn set(&mut self, joint: Joint, rotation: Quat) {
        self.rot[joint.index()] = rotation;
    }

    /// Per-joint slerp (and lerp for the offsets) toward `target`; `t` in `0..=1`.
    pub fn blend_toward(&mut self, target: &Pose, t: f32) {
        for (a, b) in self.rot.iter_mut().zip(target.rot.iter()) {
            *a = a.slerp(*b, t);
        }
        self.pelvis_offset = self.pelvis_offset.lerp(target.pelvis_offset, t);
        self.root_rot = self.root_rot.slerp(target.root_rot, t);
        self.root_offset = self.root_offset.lerp(target.root_offset, t);
    }
}

/// Procedural animator state: where in the stride cycle we are.
#[derive(Component, Default)]
pub struct ProceduralAnimator {
    pub phase: f32,
    pub clock: f32,
}

pub struct RigPlugin;

impl Plugin for RigPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (animate_procedural, apply_pose).chain());
    }
}

/// Spawns the rig under `parent` and returns the rig root (the old `PlayerBody` slot).
/// `extra` is added to the root, e.g. a marker component.
pub fn spawn_rig(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Entity,
    extra: impl Bundle,
) -> Entity {
    let cloth = materials.add(Color::srgb(0.65, 0.15, 0.15));
    let limb = materials.add(Color::srgb(0.55, 0.57, 0.62));
    let head = materials.add(Color::srgb(0.8, 0.62, 0.5));
    let boot = materials.add(Color::srgb(0.12, 0.1, 0.09));
    let pack = materials.add(Color::srgb(0.22, 0.28, 0.2));

    let root = commands
        .spawn((Transform::default(), Visibility::default(), Pose::default(), ProceduralAnimator::default(), extra))
        .insert(ChildOf(parent))
        .id();

    let mut entities = [Entity::PLACEHOLDER; JOINT_COUNT];
    for joint in JOINTS {
        let parent_entity = joint.parent().map_or(root, |p| entities[p.index()]);
        entities[joint.index()] = commands
            .spawn((Transform::from_translation(joint.rest_offset()), Visibility::default(), ChildOf(parent_entity)))
            .id();
    }

    for joint in JOINTS {
        let at = entities[joint.index()];
        let material = match joint {
            Joint::Pelvis | Joint::Spine | Joint::Chest => cloth.clone(),
            Joint::Head | Joint::Neck => head.clone(),
            _ => limb.clone(),
        };
        if let Some((end, radius)) = joint.bone() {
            let length = end.length();
            let mesh = meshes.add(Capsule3d::new(radius, (length - 2.0 * radius).max(0.001)));
            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_translation(end * 0.5).with_rotation(Quat::from_rotation_arc(Vec3::Y, end / length)),
                ChildOf(at),
            ));
        }
        // Hands, feet and a backpack so facing and animation read at a glance. +Z is the back.
        match joint {
            Joint::HandL | Joint::HandR => {
                commands.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.05))),
                    MeshMaterial3d(head.clone()),
                    Transform::default(),
                    ChildOf(at),
                ));
            }
            Joint::FootL | Joint::FootR => {
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.12, 0.05, 0.28))),
                    MeshMaterial3d(boot.clone()),
                    Transform::from_xyz(0.0, -0.025, -0.07),
                    ChildOf(at),
                ));
            }
            Joint::Chest => {
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.28, 0.38, 0.14))),
                    MeshMaterial3d(pack.clone()),
                    Transform::from_xyz(0.0, 0.1, 0.17),
                    ChildOf(at),
                ));
            }
            _ => {}
        }
    }

    commands.entity(root).insert(Rig { joints: entities });
    root
}

/// Writes the pose onto the joint transforms and the rig root.
fn apply_pose(rigs: Query<(Entity, &Rig, &Pose)>, mut transforms: Query<&mut Transform>) {
    for (root, rig, pose) in &rigs {
        if let Ok(mut t) = transforms.get_mut(root) {
            t.rotation = pose.root_rot;
            t.translation = pose.root_offset;
        }
        for joint in JOINTS {
            if let Ok(mut t) = transforms.get_mut(rig.joints[joint.index()]) {
                t.rotation = pose.rot[joint.index()];
                t.translation = joint.rest_offset();
                if joint == Joint::Pelvis {
                    t.translation += pose.pelvis_offset;
                }
            }
        }
    }
}

fn rx(angle: f32) -> Quat {
    Quat::from_rotation_x(angle)
}

/// Builds the target pose from the locomotion state and blends the rig toward it.
fn animate_procedural(
    time: Res<Time>,
    parents: Query<(&Locomotion, &KinematicBody, &GroundSensor, &LocomotionTuning, &Loadout, &Transform), Without<Pose>>,
    mut rigs: Query<(&ChildOf, &mut Pose, &mut ProceduralAnimator)>,
) {
    let dt = time.delta_secs();
    for (child_of, mut pose, mut anim) in &mut rigs {
        let Ok((loco, body, ground, tuning, loadout, parent_transform)) = parents.get(child_of.parent()) else {
            continue;
        };
        anim.clock += dt;

        // Horizontal speed, and whether it is forward or backward relative to the facing.
        let horizontal = Vec3::new(body.velocity.x, 0.0, body.velocity.z);
        let speed = horizontal.length();
        let local = parent_transform.rotation.inverse() * horizontal;
        let direction = if -local.z >= -0.1 * speed { 1.0 } else { -1.0 };

        let state = loco.state;
        let stance = state.stance();
        let airborne = matches!(state, LocomotionState::Jump | LocomotionState::Fall { .. }) || !ground.grounded;
        let reaching = matches!(state, LocomotionState::Traversal(_));
        let stride = match stance {
            Stance::Standing => 1.5,
            Stance::Crouching => 1.0,
            Stance::Prone => 0.7,
        };
        anim.phase = (anim.phase + direction * speed * dt / stride * TAU).rem_euclid(TAU);

        let mut target = Pose::REST;
        let gait_amount = (speed / 5.5).clamp(0.0, 1.0);
        let swing = anim.phase.sin() * 0.85 * gait_amount;
        let swing_other = (anim.phase + PI).sin() * 0.85 * gait_amount;
        let knee = |s: f32| -((s + 0.4).max(0.0) * 0.9 + 0.05 * gait_amount);

        // Legs: hip swing with the knee bending as the foot comes through.
        target.set(Joint::HipL, rx(swing));
        target.set(Joint::KneeL, rx(knee(swing_other)));
        target.set(Joint::HipR, rx(swing_other));
        target.set(Joint::KneeR, rx(knee(swing)));

        // Arms swing against the legs; torso leans into the run and counter-rotates a little.
        let arm = 0.7 * gait_amount;
        target.set(Joint::ShoulderL, rx(-swing * arm / 0.85));
        target.set(Joint::ShoulderR, rx(-swing_other * arm / 0.85));
        let elbow = 0.25 + 0.6 * gait_amount;
        target.set(Joint::ElbowL, rx(elbow));
        target.set(Joint::ElbowR, rx(elbow));
        let lean = 0.05 + 0.18 * gait_amount * if matches!(state, LocomotionState::Grounded { gait: Gait::Sprint, .. }) { 1.4 } else { 1.0 };
        target.set(Joint::Spine, rx(lean * 0.5) * Quat::from_rotation_y(swing * 0.12));
        target.set(Joint::Chest, rx(lean * 0.5) * Quat::from_rotation_y(-swing * 0.12));
        target.set(Joint::Head, rx(-lean * 0.7));

        // Idle breathing.
        let breath = (anim.clock * 1.6).sin();
        target.set(Joint::Chest, target.rot[Joint::Chest.index()] * rx(breath * 0.012));
        target.pelvis_offset.y += breath * 0.004;

        // Stance: lower the pelvis by bending hips and knees symmetrically.
        let drop = (tuning.standing_height - tuning.height(stance)).max(0.0) * 0.9;
        if stance == Stance::Crouching && drop > 0.0 {
            let leg = 0.83;
            let theta = (1.0 - drop / leg).clamp(-1.0, 1.0).acos();
            target.pelvis_offset.y -= drop;
            for (hip, knee_joint) in [(Joint::HipL, Joint::KneeL), (Joint::HipR, Joint::KneeR)] {
                target.set(hip, target.rot[hip.index()] * rx(theta));
                target.set(knee_joint, target.rot[knee_joint.index()] * rx(-2.0 * theta));
            }
            target.set(Joint::Spine, target.rot[Joint::Spine.index()] * rx(0.35));
            target.set(Joint::Head, rx(-0.4));
        }
        if stance == Stance::Prone {
            // Lie face down along the forward axis, arms forward.
            target.root_rot = rx(-FRAC_PI_2);
            target.root_offset = Vec3::new(0.0, 0.14, 0.0);
            target.set(Joint::ShoulderL, rx(PI * 0.9));
            target.set(Joint::ShoulderR, rx(PI * 0.9));
            target.set(Joint::Head, rx(0.7));
            let crawl = anim.phase.sin() * 0.35 * (speed / 1.5).clamp(0.0, 1.0);
            target.set(Joint::HipL, rx(crawl));
            target.set(Joint::HipR, rx(-crawl));
        }

        // Aiming: both arms up toward the weapon.
        if loadout.aiming && stance != Stance::Prone {
            target.set(Joint::ShoulderR, rx(1.35));
            target.set(Joint::ElbowR, rx(0.35));
            target.set(Joint::ShoulderL, rx(1.2) * Quat::from_rotation_z(-0.25));
            target.set(Joint::ElbowL, rx(0.6));
            target.set(Joint::Head, rx(0.0));
        }

        // Airborne and traversal.
        if airborne && !reaching {
            target.set(Joint::HipL, rx(0.55));
            target.set(Joint::KneeL, rx(-0.9));
            target.set(Joint::HipR, rx(-0.25));
            target.set(Joint::KneeR, rx(-0.5));
            target.set(Joint::ShoulderL, rx(0.5) * Quat::from_rotation_z(0.5));
            target.set(Joint::ShoulderR, rx(0.5) * Quat::from_rotation_z(-0.5));
        }
        if reaching {
            target.set(Joint::ShoulderL, rx(2.6));
            target.set(Joint::ShoulderR, rx(2.6));
            target.set(Joint::HipL, rx(0.9));
            target.set(Joint::KneeL, rx(-1.5));
            target.set(Joint::HipR, rx(-0.2));
            target.set(Joint::KneeR, rx(-0.4));
        }

        let t = 1.0 - (-14.0 * dt).exp();
        pose.blend_toward(&target, t);
    }
}
