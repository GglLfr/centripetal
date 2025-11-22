use crate::{
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
    pub fn execute(world: &mut World) {
        world.resource_scope(|world, this: Mut<Self>| {
            let this = this.into_inner();
            let mut captured_resources = Vec::new();
            let mut captured_components = VecBelt::new(256);

            ComputeTaskPool::get().scope(|scope| {
                scope.spawn(async { this.resource_capturer.run_readonly(&mut captured_resources, world).unwrap() });
                for component_capturer in &mut this.component_capturers {
                    scope.spawn(async { component_capturer.run_readonly(&captured_components, world).unwrap() });
                }
            });

            // Spawn a new task, cancelling previous save tasks.
            let app_registry = world.resource::<AppTypeRegistry>().clone();
            this.ongoing_task = Some(IoTaskPool::get().spawn(async move {
                let data = SaveData {
                    resources: captured_resources,
                    entities: captured_components.clear(|slice| {
                        let mut tree = BTreeMap::<Entity, Vec<Box<dyn Reflect>>>::new();
                        for (e, component) in slice {
                            tree.entry(e).or_default().push(component);
                        }
                        tree
                    }),
                };

                futures_lite::future::yield_now().await;

                // The RAII guard of the read-write lock needs to not cross `await` boundary, so confine it.
                let output = {
                    let registry = &*app_registry.read();
                    let serializer = SaveDataSerializer { registry, data: &data };
                    ron::ser::to_string_pretty(&serializer, default()).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                };

                futures_lite::future::yield_now().await;

                // TODO output to a file.
                println!("{output}");
                Ok(())
            }));
        })
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
