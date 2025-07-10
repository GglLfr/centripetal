use bevy::{
    ecs::{
        archetype::Archetype,
        component::Tick,
        query::{QueryData, QueryItem, QuerySingleError},
        system::{SystemMeta, SystemParam, SystemParamValidationError},
        world::{DeferredWorld, unsafe_world_cell::UnsafeWorldCell},
    },
    platform::sync::{Mutex, PoisonError},
    prelude::*,
    render::camera::ScalingMode,
};

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

#[derive(Deref, DerefMut)]
pub struct CameraQuery<'w, D: QueryData> {
    inner: QueryItem<'w, D>,
}

impl<'w, D: QueryData> CameraQuery<'w, D> {
    pub fn into_inner(self) -> QueryItem<'w, D> {
        self.inner
    }
}

unsafe impl<D: 'static + QueryData> SystemParam for CameraQuery<'_, D> {
    // HACK: Uses `Mutex` because `Query::get_param` requires a mutable state, and
    // `QueryState::query_unchecked_manual_with_ticks` needs `SystemMeta::last_run` which is
    // *annoyingly* private.
    type State = Mutex<<Query<'static, 'static, D, With<MainCamera>> as SystemParam>::State>;
    type Item<'world, 'state> = CameraQuery<'world, D>;

    fn init_state(world: &mut World, system_meta: &mut SystemMeta) -> Self::State {
        Mutex::new(Query::init_state(world, system_meta))
    }

    unsafe fn get_param<'world, 'state>(
        state: &'state mut Self::State,
        system_meta: &SystemMeta,
        world: UnsafeWorldCell<'world>,
        change_tick: Tick,
    ) -> Self::Item<'world, 'state> {
        let query = unsafe {
            Query::get_param(
                state.get_mut().unwrap_or_else(PoisonError::into_inner),
                system_meta,
                world,
                change_tick,
            )
        };

        CameraQuery {
            inner: query
                .single_inner()
                .expect("The query was expected to contain exactly one matching entity."),
        }
    }

    unsafe fn new_archetype(state: &mut Self::State, archetype: &Archetype, system_meta: &mut SystemMeta) {
        unsafe {
            Query::new_archetype(
                state.get_mut().unwrap_or_else(PoisonError::into_inner),
                archetype,
                system_meta,
            )
        };
    }

    fn apply(state: &mut Self::State, system_meta: &SystemMeta, world: &mut World) {
        Query::apply(state.get_mut().unwrap_or_else(PoisonError::into_inner), system_meta, world);
    }

    fn queue(state: &mut Self::State, system_meta: &SystemMeta, world: DeferredWorld) {
        Query::queue(state.get_mut().unwrap_or_else(PoisonError::into_inner), system_meta, world);
    }

    unsafe fn validate_param(
        state: &Self::State,
        system_meta: &SystemMeta,
        world: UnsafeWorldCell,
    ) -> Result<(), SystemParamValidationError> {
        let mut lock = state.lock().unwrap_or_else(PoisonError::into_inner);
        let query = unsafe { Query::get_param(&mut *lock, system_meta, world, world.change_tick()) };

        match query.single_inner() {
            Ok(..) => Ok(()),
            Err(QuerySingleError::NoEntities(e)) | Err(QuerySingleError::MultipleEntities(e)) => {
                Err(SystemParamValidationError::invalid::<Self>(e))
            }
        }
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
    Camera {
        clear_color: ClearColorConfig::Custom(Color::NONE),
        ..default()
    }
)]
pub struct MainCamera;

pub fn startup_camera(mut commands: Commands) {
    debug!("Spawned `MainCamera` as entity {}!", commands.spawn(MainCamera).id());
}

pub fn move_camera(
    camera: CameraQuery<(&mut Transform, &Projection)>,
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
            let cam_bounds = ortho.area.size();
            let bounds = ***level_bounds;
            let size_diff = bounds - cam_bounds;

            let x = if size_diff.x < 0. {
                bounds.x / 2.
            } else {
                trns.x.clamp(cam_bounds.x / 2., bounds.x - cam_bounds.x / 2.)
            };

            let y = if size_diff.y < 0. {
                bounds.y / 2.
            } else {
                trns.y.clamp(cam_bounds.y / 2., bounds.y - cam_bounds.y / 2.)
            };

            vec2(x, y)
        }
        CameraConfines::Fixed(rel) => trns + rel,
    }
    .extend(camera_trns.translation.z);

    Ok(())
}
