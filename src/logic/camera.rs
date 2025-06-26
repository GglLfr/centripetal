use bevy::{prelude::*, render::camera::ScalingMode};

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(Transform, CameraConfines, CameraScale)]
pub struct CameraTarget;

#[derive(Debug, Copy, Clone, Default, Component)]
pub enum CameraConfines {
    #[default]
    Level,
    Fixed(Vec2),
}

#[derive(Debug, Copy, Clone, Component, Deref, DerefMut)]
pub struct CameraScale(pub Vec2);
impl Default for CameraScale {
    fn default() -> Self {
        Self(Vec2::splat(1.))
    }
}

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(
    Camera2d,
    Projection::Orthographic(OrthographicProjection {
        scaling_mode: ScalingMode::AutoMax {
            max_width: 1920.,
            max_height: 1080.,
        },
        scale: 1. / 3.,
        ..OrthographicProjection::default_2d()
    }),
)]
pub struct MainCamera;

pub fn startup_camera(mut commands: Commands) {
    commands.spawn(MainCamera);
}

pub fn move_camera(
    mut camera: Single<&mut Transform, With<MainCamera>>,
    target: Single<(&Transform, Option<&ChildOf>), (With<CameraTarget>, Without<MainCamera>)>,
    child_of_query: Query<(&Transform, Option<&ChildOf>), Without<MainCamera>>,
) {
    let (target_trns, target_child_of) = *target;
    let mut trns = *target_trns;

    if let Some(mut e) = target_child_of.map(ChildOf::parent) {
        while let Ok((&parent_trns, child_of)) = child_of_query.get(e) {
            trns = parent_trns * trns;
            if let Some(parent) = child_of.map(ChildOf::parent) {
                e = parent;
            } else {
                break
            }
        }
    }

    **camera = trns;
}
