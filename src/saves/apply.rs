use crate::{ReflectMapAssetIds, prelude::*, saves::SaveData, util::AsyncContext};

pub fn apply_save(
    server: AssetServer,
    registry: AppTypeRegistry,
    ctx: AsyncContext,
    path: impl AsRef<Path>,
) -> impl ConditionalSendFuture<Output = Result<()>> {
    let path = AssetPath::from_path(path.as_ref()).into_owned().with_source("data");
    async move {
        struct EntityMap<'a> {
            errors: &'a mut Vec<String>,
            entity_map: &'a mut EntityHashMap<Entity>,
        }

        impl EntityMapper for EntityMap<'_> {
            fn get_mapped(&mut self, source: Entity) -> Entity {
                match self.entity_map.get(&source) {
                    Some(&target) => target,
                    None => {
                        self.errors.push(format!("Unmapped entity {source}: ensure that *every* components that serialize entities are included in the `map_entities()` implementation, or is `#[reflect(ignore)]`-ed."));
                        Entity::PLACEHOLDER
                    }
                }
            }

            fn set_mapped(&mut self, source: Entity, target: Entity) {
                self.entity_map.insert(source, target);
            }
        }

        let mut errors = Vec::<String>::new();
        let mut file = ctx
            .asset
            .send_typed::<SaveData>({
                let handle = server.load::<SaveData>(path);
                server.wait_for_asset(&handle).await?;
                handle.untyped()
            })
            .await?;

        let path_to_ids = ctx.asset_path.recv().await?;
        let mut commands = ctx.commands();

        let mut entity_map = EntityHashMap::with_capacity(file.entities.len());
        let new_entities = commands.spawn_many(file.entities.len() as u32).await?.into_iter();
        for ((&entity, ..), mapped_entity) in file.entities.iter().zip(new_entities) {
            entity_map.insert(entity, mapped_entity);
        }

        let mut yield_now = 0u8;
        for (.., components) in &mut file.entities {
            // Ensure the read-write lock guard doesn't cross `await` boundaries to make the future `Send`.
            {
                let registry = registry.read();
                for component in components.iter_mut() {
                    // Can't use `Any::type_id` here because it would require the borrow to be static.
                    let component_type_id = component.reflect_type_info().type_id();
                    if let Some(map_asset_ids) = registry.get_type_data::<ReflectMapAssetIds>(component_type_id) {
                        map_asset_ids.map_asset_ids(&mut **component, &mut |id| match id {
                            UntypedAssetId::Index { type_id, index } => {
                                let Some(path) = file.assets.get(&type_id).and_then(|type_paths| type_paths.get(&index)) else {
                                    errors.push(format!("Couldn't map {id} into a known asset path, caused by malformed `SaveData`."));
                                    return id
                                };

                                let Some(&id) = path_to_ids.get(path) else {
                                    errors.push(format!(
                                        "Couldn't map {path} into a known asset ID; were some asset file locations refactored?"
                                    ));
                                    return id
                                };
                                id
                            }
                            UntypedAssetId::Uuid { .. } => id,
                        });
                    }

                    if !errors.is_empty() {
                        Err(errors.join("\n"))?
                    }

                    if let Some(component_fns) = registry.get_type_data::<ReflectComponent>(component_type_id) {
                        component_fns.map_entities(&mut **component, &mut EntityMap {
                            errors: &mut errors,
                            entity_map: &mut entity_map,
                        });
                    }

                    if !errors.is_empty() {
                        Err(errors.join("\n"))?
                    }
                }
            }

            // If adding an integer results in a *lesser* value, then a wrapping has occurred.
            // Cooperatively yield a time slice to allow other tasks to run.
            let next_yield_now = yield_now.wrapping_add((components.len() % 256) as u8);
            if next_yield_now < mem::replace(&mut yield_now, next_yield_now) {
                futures_lite::future::yield_now().await
            }
        }

        for (e, components) in file.entities {
            commands
                .entity(*entity_map.get(&e).expect("Entity map contains everything"))
                .insert_raw_bundle(components, RelationshipHookMode::RunIfNotLinked);
        }

        commands.submit().await
    }
}
