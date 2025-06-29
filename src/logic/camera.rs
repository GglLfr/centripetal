use bevy::{prelude::*, render::camera::ScalingMode};

use crate::logic::LevelBounds;

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
    camera: Single<(&mut Transform, &Projection), With<MainCamera>>,
    target: Single<(&Transform, &CameraConfines, Option<&ChildOf>), (With<CameraTarget>, Without<MainCamera>)>,
    level_bounds: Single<&LevelBounds>,
    child_of_query: Query<(&Transform, Option<&ChildOf>), Without<MainCamera>>,
) -> Result {
    let (mut camera_trns, camera_proj) = camera.into_inner();
    let Projection::Orthographic(ortho) = camera_proj else {
        Err("Camera projection must be orthographic")?
    };

    let (target_trns, &target_confines, target_child_of) = target.into_inner();
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

    let trns = trns.translation.truncate();
    camera_trns.translation = match target_confines {
        CameraConfines::Level => {
            let mut rect = ortho.area;
            rect.min += trns;
            rect.max += trns;

            let bounds = ***level_bounds;
            let size_diff = bounds - rect.size();

            let x = if size_diff.x < 0. { bounds.x / 2. } else { trns.x.clamp(0., size_diff.x) };
            let y = if size_diff.y < 0. { bounds.y / 2. } else { trns.y.clamp(0., size_diff.y) };

            vec2(x, y)
        }
        CameraConfines::Fixed(rel) => trns + rel,
    }
    .extend(camera_trns.translation.z);

    Ok(())
}
