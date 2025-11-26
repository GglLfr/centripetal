use crate::prelude::*;

fn quat_to_yaw(q: Quat) -> f32 {
    let rx = q.mul_vec3(Vec3::X);
    let proj_len2 = rx.x * rx.x + rx.y * rx.y;

    if proj_len2 > 1e-8 {
        rx.y.atan2(rx.x)
    } else {
        let ry = q.mul_vec3(Vec3::Y);
        if (ry.x * ry.x + ry.y * ry.y) > 1e-8 { ry.y.atan2(ry.x) } else { 0. }
    }
}

#[derive(Reflect, Component, Debug, Clone, Copy, PartialEq)]
#[require(Transform, GlobalTransform2d)]
#[reflect(Component, Debug, Default, FromWorld, Clone, PartialEq)]
pub struct Transform2d {
    // The Z component isn't actually used, but it's useful for draw ordering.
    pub translation: Vec3,
    pub rotation: f32,
    pub scale: Vec2,
}

impl Default for Transform2d {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl From<Transform> for Transform2d {
    fn from(value: Transform) -> Self {
        Self {
            translation: value.translation,
            rotation: quat_to_yaw(value.rotation),
            scale: value.scale.truncate(),
        }
    }
}

impl Transform2d {
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: 0.,
        scale: Vec2::ONE,
    };

    pub const ABOVE: Self = Self {
        translation: Vec3::new(0., 0., f32::EPSILON),
        ..Self::IDENTITY
    };
}

#[derive(Reflect, Component, Default, Debug, Clone, Copy, PartialEq, Deref, DerefMut)]
#[component(on_insert = validate_parent_has_component::<GlobalTransform>)]
#[reflect(Component, Debug, Default, FromWorld, Clone, PartialEq)]
pub struct GlobalTransform2d {
    #[deref]
    pub affine: Affine2,
    pub z: f32,
}

impl GlobalTransform2d {
    pub const IDENTITY: Self = Self {
        affine: Affine2::IDENTITY,
        z: 0.,
    };
}

impl From<GlobalTransform> for GlobalTransform2d {
    fn from(value: GlobalTransform) -> Self {
        let (scale, rotation, translation) = value.to_scale_rotation_translation();
        Self {
            affine: Affine2::from_scale_angle_translation(scale.truncate(), quat_to_yaw(rotation), translation.truncate()),
            z: translation.z,
        }
    }
}

fn sync_to_local_3d(mut transforms: Query<(&mut Transform, &mut Transform2d)>) {
    transforms.par_iter_mut().for_each(|(dst, mut src)| {
        if src.is_changed() {
            let dst = dst.into_inner();
            dst.translation = src.translation;
            dst.rotation = Quat::from_axis_angle(Vec3::Z, src.rotation);
            dst.scale = src.scale.extend(dst.scale.z);
        } else if dst.is_changed() {
            *src = (*dst).into();
        }
    });
}

fn writeback_to_global_2d(mut transforms: Query<(&GlobalTransform, &mut GlobalTransform2d), Changed<GlobalTransform>>) {
    transforms.par_iter_mut().for_each(|(&src, mut dst)| {
        *dst = src.into();
    });
}

pub(super) fn plugin(app: &mut App) {
    use bevy::transform::systems::*;
    app.add_systems(
        PostStartup,
        (
            // Add to the `TransformSystems::Propagate` chain. Always remember to adjust for changes when updating Bevy.
            sync_to_local_3d.before(mark_dirty_trees),
            writeback_to_global_2d.after(sync_simple_transforms),
        )
            .in_set(TransformSystems::Propagate),
    )
    .add_systems(
        PostUpdate,
        (
            sync_to_local_3d.before(mark_dirty_trees),
            writeback_to_global_2d.after(sync_simple_transforms),
        )
            .in_set(TransformSystems::Propagate),
    );
}
