use crate::{prelude::*, util::ecs::ReflectComponentPtr};

fn quat_to_yaw(q: Quat) -> Rot2 {
    let rx = q.mul_vec3(Vec3::X);
    let len2 = rx.x * rx.x + rx.y * rx.y;

    if len2 > 1e-8 {
        let len = len2.sqrt().recip();
        Rot2 {
            cos: rx.x * len,
            sin: rx.y * len,
        }
    } else {
        let ry = q.mul_vec3(Vec3::Y);
        let len2 = ry.x * ry.x + ry.y * ry.y;

        if len2 > 1e-8 {
            let len = len2.sqrt().recip();
            Rot2 {
                cos: ry.x * len,
                sin: ry.y * len,
            }
        } else {
            Rot2::IDENTITY
        }
    }
}

#[derive(Reflect, Component, Debug, Clone, Copy, PartialEq)]
#[require(Transform, GlobalTransform2d)]
#[reflect(Component, ComponentPtr, Debug, Default, FromWorld, Clone, PartialEq)]
pub struct Transform2d {
    // The Z component isn't actually used, but it's useful for draw ordering.
    pub translation: Vec3,
    pub rotation: Rot2,
    pub scale: Vec2,
}

impl Default for Transform2d {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mul for Transform2d {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let z = self.translation.z + rhs.translation.z;
        Self {
            translation: (self.translation.truncate() + self.rotation * self.scale * rhs.translation.truncate()).extend(z),
            rotation: self.rotation * rhs.rotation,
            scale: self.scale * rhs.scale,
        }
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
        rotation: Rot2::IDENTITY,
        scale: Vec2::ONE,
    };

    pub const ABOVE: Self = Self {
        translation: Vec3::new(0., 0., f32::EPSILON),
        ..Self::IDENTITY
    };

    pub const fn from_xy(x: f32, y: f32) -> Self {
        Self::from_xyz(x, y, 0.)
    }

    pub const fn from_xyz(x: f32, y: f32, z: f32) -> Self {
        Self {
            translation: vec3(x, y, z),
            ..Self::IDENTITY
        }
    }

    pub const fn from_translation(translation: Vec3) -> Self {
        Self {
            translation,
            ..Self::IDENTITY
        }
    }

    pub fn affine(self) -> Affine2 {
        let Rot2 { cos, sin } = self.rotation;
        let rotation = Mat2::from_cols(vec2(cos, sin), vec2(-sin, cos));
        Affine2 {
            matrix2: Mat2::from_cols(rotation.x_axis * self.scale.x, rotation.y_axis * self.scale.y),
            translation: self.translation.truncate(),
        }
    }

    pub fn affine_and_z(self) -> (Affine2, f32) {
        (self.affine(), self.translation.z)
    }
}

#[derive(Reflect, Component, Default, Debug, Clone, Copy, PartialEq, Deref, DerefMut)]
#[component(on_insert = validate_parent_has_component::<GlobalTransform>)]
#[reflect(Component, ComponentPtr, Debug, Default, FromWorld, Clone, PartialEq)]
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
        Self::from_scale_rotation_translation(scale.truncate(), quat_to_yaw(rotation), translation)
    }
}

impl GlobalTransform2d {
    pub fn from_scale_rotation_translation(scale: Vec2, rotation: Rot2, translation: Vec3) -> Self {
        let (affine, z) = Transform2d {
            scale,
            rotation,
            translation,
        }
        .affine_and_z();
        Self { affine, z }
    }
}

fn sync_to_local_3d(mut transforms: Query<(&mut Transform, &mut Transform2d)>) {
    transforms.par_iter_mut().for_each(|(dst, mut src)| {
        if src.is_changed() {
            let dst = dst.into_inner();
            dst.translation = src.translation;
            dst.rotation = Quat::from_axis_angle(Vec3::Z, src.rotation.as_radians());
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
