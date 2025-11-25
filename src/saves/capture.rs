use crate::{
    DATA_SOURCE, KnownAssets, ReflectMapAssetIds,
    prelude::*,
    saves::{ReflectSave, SaveData, SaveDataSerializer},
};

#[derive(Resource)]
pub struct SaveCapturer {
    ongoing_task: Option<Task<io::Result<()>>>,
    resource_capturer: BoxedReadOnlySystem<InMut<'static, Vec<Box<dyn Reflect>>>>,
    component_capturers: Vec<BoxedReadOnlySystem<InRef<'static, VecBelt<(Entity, Box<dyn Reflect>)>>>>,
}

impl SaveCapturer {
    pub fn capture(path: impl Into<PathBuf>) -> impl Command {
        let mut captured_resources = Vec::new();
        let mut captured_components = VecBelt::new(256);
        let path = AssetPath::from_path_buf(path.into()).with_source(DATA_SOURCE);

        move |world: &mut World| {
            world.resource_scope(|world, this: Mut<Self>| {
                let this = this.into_inner();
                let id_to_path = world.resource::<KnownAssets>().id_to_path().clone();
                let registry = world.resource::<AppTypeRegistry>().clone();
                let server = world.resource::<AssetServer>().clone();

                ComputeTaskPool::get().scope(|scope| {
                    scope.spawn(async { this.resource_capturer.run_readonly(&mut captured_resources, world).unwrap() });
                    for component_capturer in &mut this.component_capturers {
                        scope.spawn(async { component_capturer.run_readonly(&captured_components, world).unwrap() });
                    }
                });

                // Spawn a new task after ensuring the previous one is finished.
                info!("Saving world to {}", path.path().display());
                let previous_task = this.ongoing_task.take();
                this.ongoing_task = Some(IoTaskPool::get().spawn(async move {
                    if let Some(task) = previous_task
                        && let Err(e) = task.await
                    {
                        error!("Couldn't save world: {e}")
                    }

                    let (assets, entities) = captured_components.clear(|slice| {
                        let mut assets = BTreeMap::<TypeId, BTreeMap<_, _>>::new();
                        let mut tree = BTreeMap::<Entity, Vec<_>>::new();

                        let registry = &*registry.read();
                        for (e, component) in slice {
                            if let Some(data) = registry.get_type_data::<ReflectMapAssetIds>(component.reflect_type_info().type_id()) {
                                data.visit_asset_ids(&*component, &mut |id| {
                                    if let UntypedAssetId::Index { type_id, index } = id {
                                        let Some(path) = id_to_path.get(&id) else {
                                            warn!("While trying to save, asset {id} was found to be impossible to be reliably persisted.");
                                            return
                                        };

                                        assets.entry(type_id).or_default().insert(index, path.clone());
                                    }
                                });
                            }

                            tree.entry(e).or_default().push(component);
                        }

                        (assets, tree)
                    });

                    let data = SaveData {
                        assets,
                        resources: captured_resources,
                        entities,
                    };

                    futures_lite::future::yield_now().await;
                    let output = {
                        let registry = &*registry.read();
                        let serializer = SaveDataSerializer { registry, data: &data };

                        ron::ser::to_string_pretty(&serializer, default()).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                    };

                    futures_lite::future::yield_now().await;
                    let writer = server
                        .get_source(path.source())
                        .map_err(|e| io::Error::new(io::ErrorKind::NotFound, e))?
                        .writer()
                        .map_err(|e| io::Error::new(io::ErrorKind::NotFound, e))?;

                    let fs_path = path.path();
                    if let Some(parent) = fs_path.parent() {
                        writer.create_directory(parent).await.map_err(|AssetWriterError::Io(e)| e)?;
                    }
                    writer
                        .write_bytes(path.path(), output.as_bytes())
                        .await
                        .map_err(|AssetWriterError::Io(e)| e)?;

                    let backup_path = path.path().with_added_extension("bak");
                    if let Err(AssetWriterError::Io(e)) = writer.write_bytes(&backup_path, output.as_bytes()).await {
                        _ = writer.remove(&backup_path).await;
                        error!("Couldn't save backup file to {}: {e}", backup_path.display());
                    }

                    info!("Successfully saved the world to {}", path.path().display());
                    Ok(())
                }));
            })
        }
    }
}

fn build_capturers(world: &mut World) {
    world.flush();

    let registry = world.resource::<AppTypeRegistry>().clone();
    let mut component_capturers = Vec::new();
    let mut resources = Vec::new();

    for registration in registry.read().iter() {
        if registration.data::<ReflectSave>().is_none() {
            continue
        }

        if let Some(component_fns) = registration.data::<ReflectComponent>().cloned() {
            let id = component_fns.register_component(world);
            let name = world.components().get_info(id).expect("Component was just registered").name();

            component_capturers.push(Box::new(
                (QueryParamBuilder::new::<FilteredEntityRef, Allow<Disabled>>(|builder| {
                    builder.ref_id(id);
                }),)
                    .build_state(world)
                    .build_any_system(
                        move |InRef(components): InRef<VecBelt<(Entity, Box<dyn Reflect>)>>, query: Query<FilteredEntityRef, Allow<Disabled>>| {
                            // Don't `par_iter()` here, because each queries will be parallelized anyway.
                            // Avoid creating too many tasks.
                            for e in query {
                                let id = e.id();
                                if let Some(reflected) = component_fns.reflect(e) {
                                    match reflected.reflect_clone() {
                                        Ok(cloned) => {
                                            components.append([(id, cloned)]);
                                        }
                                        Err(e) => {
                                            warn!("While trying to save, component {name} of entity {id} couldn't be cloned: {e}");
                                        }
                                    }
                                }
                            }
                        },
                    ),
            ) as _);
        } else if let Some(resource_fns) = registration.data::<ReflectResource>().cloned() {
            // Coalesce all resources into a single system.
            let id = resource_fns.register_resource(world);
            let name = world.components().get_info(id).expect("Resource was just registered").name();
            resources.push((id, name, resource_fns));
        }
    }

    let resource_capturer = Box::new(
        (FilteredResourcesParamBuilder::new(|builder| {
            for &(id, ..) in &resources {
                builder.add_read_by_id(id);
            }
        }),)
            .build_state(world)
            .build_any_system(move |InMut(captured): InMut<Vec<Box<dyn Reflect>>>, access: FilteredResources| {
                captured.extend(resources.iter().flat_map(|(.., name, resource_fns)| {
                    let reflected = resource_fns.reflect(access).ok()?;
                    reflected
                        .reflect_clone()
                        .inspect_err(|e| warn!("While trying to save, resource {name} couldn't be cloned: {e}"))
                        .ok()
                }));
            }),
    );

    world.insert_resource(SaveCapturer {
        ongoing_task: None,
        resource_capturer,
        component_capturers,
    });
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, build_capturers);
}
