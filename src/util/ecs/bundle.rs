use bevy::{
    ecs::{bundle::BundleFromComponents, component::ComponentsRegistrator, world::WorldId},
    ptr::{OwningPtr, PtrMut},
};

use crate::{
    prelude::*,
    util::ecs::{ReflectComponentPtr, ReflectComponentPtrFns},
};

pub struct RawBundle {
    world_id: WorldId,
    components: HashMap<ComponentId, (ReflectComponentPtrFns, *mut ())>,
}

impl RawBundle {
    #[track_caller]
    pub fn insert(
        components_iter: impl IntoIterator<Item = Box<dyn Reflect>> + 'static + Send + Sync,
        hook_mode: RelationshipHookMode,
    ) -> impl EntityCommand<Result> {
        move |mut entity: EntityWorldMut| {
            let this = entity.world_scope(|world| Self::from_reflect(world, components_iter))?;
            entity.resource_scope(|entity, bundles: Mut<PartialBundles>| bundles.insert(entity, this, hook_mode));
            Ok(())
        }
    }

    pub fn from_reflect(world: &mut World, components_iter: impl IntoIterator<Item = Box<dyn Reflect>>) -> Result<Self> {
        let components_iter = components_iter.into_iter();
        let mut components = HashMap::with_capacity(match components_iter.size_hint() {
            (lower_bound, None) => lower_bound,
            (.., Some(upper_bound)) => upper_bound,
        });

        let registry = world.resource::<AppTypeRegistry>().clone();
        for component in components_iter {
            let type_info = component.reflect_type_info();
            let component_fns = registry
                .read()
                .get_type_data::<ReflectComponentPtr>(type_info.type_id())
                .ok_or_else(|| format!("Missing `ReflectComponentPtr` for {}", type_info.type_path()))?
                .component_fns();

            let component_id = (component_fns.register_component)(world);
            let component =
                (component_fns.into_owned_ptr)(component).map_err(|_| format!("Missing `ReflectComponentPtr` for {}", type_info.type_path()))?;

            if let Some((fns, ptr)) = components.insert(component_id, (component_fns, component)) {
                // Safety: `ptr` was obtained through `into_owned_ptr` and hasn't been consumed.
                unsafe { (fns.drop_ptr)(ptr) }
            }
        }

        Ok(Self {
            world_id: world.id(),
            components,
        })
    }
}

/// Information to partially extract statically known bundles from component pointers, to ensure
/// their component hooks are only run after all component transactions have been done.
///
/// This is hopefully temporary; for whatever reason Bevy deliberately hides dynamic insertions
/// *with component hooks* from user-facing APIs.
///
/// Bundles registered here are meant to be disjoint with each other.
#[derive(Resource)]
pub struct PartialBundles {
    info: HashMap<Box<[ComponentId]>, PartialBundleInfo>,
}

impl PartialBundles {
    pub fn add<T: Bundle + BundleFromComponents>(&mut self, registrator: &mut ComponentsRegistrator) {
        unsafe fn insert<T: Bundle + BundleFromComponents>(
            entity: &mut EntityWorldMut,
            hook_mode: RelationshipHookMode,
            mut ctx: PtrMut,
            component_order: &[ComponentId],
            component_extractor: unsafe fn(PtrMut, ComponentId) -> OwningPtr,
        ) {
            let mut component_order = component_order.iter().copied();
            let bundle = unsafe {
                T::from_components(&mut ctx, &mut |ctx| {
                    component_extractor(ctx.reborrow(), component_order.next().unwrap_unchecked())
                })
            };

            entity.insert_with_relationship_hook_mode(bundle, hook_mode);
        }

        let mut ids = Vec::new();
        T::component_ids(registrator, &mut |id| {
            ids.push(id);
        });

        self.info.insert(ids.into_boxed_slice(), PartialBundleInfo { insert: insert::<T> });
    }

    pub fn insert(&self, entity: &mut EntityWorldMut, mut bundle: RawBundle, hook_mode: RelationshipHookMode) {
        unsafe fn inserter(bundle: PtrMut, id: ComponentId) -> OwningPtr {
            unsafe {
                // Safety:
                // - ID existence is checked by the caller.
                // - Pointer is never null.
                // - Owning pointer is immediately consumed by bundle, and will be deallocated later.
                let bundle = bundle.deref_mut::<RawBundle>();
                let &(.., component) = bundle.components.get(&id).unwrap_unchecked();
                OwningPtr::new(NonNull::new_unchecked(component).cast())
            }
        }

        assert_eq!(
            entity.world().id(),
            bundle.world_id,
            "`RawBundle` was initialized from a different `World`",
        );

        for (ids, info) in &self.info {
            // "Does not have ID that is not contained" is equal to "contains all the IDs."
            if ids.iter().find(|&id| !bundle.components.contains_key(id)).is_none() {
                let bundle = PtrMut::from(&mut bundle);
                unsafe {
                    // Safety:
                    // - `bundle` is a `RawBundle`.
                    // - `ids` is obtained from `Bundle::component_ids`.
                    (info.insert)(entity, hook_mode, bundle, ids, inserter)
                }
            } else {
                continue
            }

            for id in ids.iter() {
                // Safety:
                // - ID existence is checked above.
                // - Pointer is allocated by `into_owned_ptr` and has been consumed by the bundle.
                unsafe {
                    let (fns, uninit_component) = bundle.components.remove(id).unwrap_unchecked();
                    (fns.drop_uninit_ptr)(uninit_component);
                }
            }
        }

        for (.., (fns, component)) in bundle.components {
            // Safety: Pointer is allocated by `into_owned_ptr` and hasn't been consumed.
            unsafe { (fns.insert_from_ptr)(component, entity, hook_mode) }
        }
    }
}

impl FromWorld for PartialBundles {
    fn from_world(#[expect(unused, reason = "No built-in partial bundles yet")] world: &mut World) -> Self {
        let this = Self { info: default() };
        this
    }
}

struct PartialBundleInfo {
    insert: unsafe fn(&mut EntityWorldMut, RelationshipHookMode, PtrMut, &[ComponentId], unsafe fn(PtrMut, ComponentId) -> OwningPtr),
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PartialBundles>();
}
