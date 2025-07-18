use std::{fs, io, path::PathBuf};

use bevy::{
    platform::sync::{LazyLock, PoisonError, RwLock},
    prelude::*,
    reflect::{DynamicTypePath, erased_serde},
};
use serde::{Deserialize, Serialize, Serializer};
use serde_flexitos::{GetError, MapRegistry, Registry, serialize_trait_object};

use crate::Dirs;

pub trait SaveResource: Resource + DynamicTypePath + erased_serde::Serialize {
    fn insert(self: Box<Self>, world: &mut World);
}

impl<T: Resource + DynamicTypePath + erased_serde::Serialize> SaveResource for T {
    fn insert(self: Box<Self>, world: &mut World) {
        world.insert_resource(*self);
    }
}

impl Serialize for dyn SaveResource {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_trait_object(serializer, self.reflect_short_type_path(), self)
    }
}

#[derive(Resource)]
pub struct SaveRegistry {
    base_dir: PathBuf,
    removals: Vec<fn(&mut World)>,
    defaults: Vec<Box<dyn FnMut(&mut World) -> Result + 'static + Send + Sync>>,
}

static REGISTRY: LazyLock<RwLock<MapRegistry<dyn SaveResource>>> =
    LazyLock::new(|| RwLock::new(MapRegistry::new("SaveResource")));

impl SaveRegistry {
    pub fn save<T: Resource + TypePath + Serialize + for<'de> Deserialize<'de>>(
        &mut self,
        mut add_default: impl FnMut(&mut World) -> Result<Option<T>> + 'static + Send + Sync,
    ) {
        REGISTRY
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .register(T::short_type_path(), |d| Ok(Box::new(erased_serde::deserialize::<T>(d)?)));

        self.removals.push(|world| {
            world.remove_resource::<T>();
        });

        self.defaults.push(Box::new(move |world| {
            if !world.contains_resource::<T>() &&
                let Some(resource) = add_default(world)?
            {
                world.insert_resource(resource);
            }

            Ok(())
        }));
    }

    pub fn apply(&mut self, world: &mut World, resources: impl IntoIterator<Item = Box<dyn SaveResource>>) -> Result {
        for &removal in &self.removals {
            removal(world);
        }

        let registry = REGISTRY.read().unwrap_or_else(PoisonError::into_inner);
        for resource in resources {
            if let Err(GetError::NotRegistered { id }) = registry.get_deserialize_fn(resource.reflect_short_type_path()) {
                Err(format!("`{id}` is not registered; call `app.save_resource::<{id}>()` first"))?
            }

            resource.insert(world);
        }

        for default in &mut self.defaults {
            default(world)?;
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct ApplySave(pub Vec<Box<dyn SaveResource>>);
impl ApplySave {
    pub fn with<T: SaveResource>(mut self, resource: impl Into<Box<T>>) -> Self {
        self.0.push(resource.into());
        self
    }
}

impl Command<Result> for ApplySave {
    fn apply(self, world: &mut World) -> Result {
        world.resource_scope(|world, mut registry: Mut<SaveRegistry>| registry.apply(world, self.0))
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct SavePlugin;
impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        let dirs = app.world().resource::<Dirs>();
        let base_dir = dirs.data.join("saves");

        match fs::create_dir(&base_dir) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => panic!("{e}"),
        }

        app.insert_resource(SaveRegistry {
            base_dir,
            removals: Vec::new(),
            defaults: Vec::new(),
        });
    }
}

pub trait SaveApp {
    fn save_resource<T: Resource + TypePath + Serialize + for<'de> Deserialize<'de>>(&mut self) -> &mut Self {
        self.save_resource_with::<T>(|_| Ok(None))
    }

    fn save_resource_init<T: Resource + FromWorld + TypePath + Serialize + for<'de> Deserialize<'de>>(
        &mut self,
    ) -> &mut Self {
        self.save_resource_with::<T>(|world| Ok(Some(T::from_world(world))))
    }

    fn save_resource_with<T: Resource + TypePath + Serialize + for<'de> Deserialize<'de>>(
        &mut self,
        add_default: impl FnMut(&mut World) -> Result<Option<T>> + 'static + Send + Sync,
    ) -> &mut Self;
}

impl SaveApp for App {
    fn save_resource_with<T: Resource + TypePath + Serialize + for<'de> Deserialize<'de>>(
        &mut self,
        add_default: impl FnMut(&mut World) -> Result<Option<T>> + 'static + Send + Sync,
    ) -> &mut Self {
        self.world_mut().resource_mut::<SaveRegistry>().save::<T>(add_default);
        self
    }
}
