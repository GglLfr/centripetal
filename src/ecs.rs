use std::{borrow::Cow, marker::PhantomData, time::Duration};

use avian2d::prelude::*;
use bevy::{
    ecs::{
        archetype::ArchetypeComponentId,
        bundle::{BundleEffect, DynamicBundle},
        component::{
            ComponentId, Components, ComponentsRegistrator, RequiredComponents, StorageType, Tick,
        },
        entity_disabling::Disabled,
        never::Never,
        query::{Access, QueryFilter},
        system::{IntoObserverSystem, RunSystemOnce, SystemParamValidationError},
        world::{DeferredWorld, unsafe_world_cell::UnsafeWorldCell},
    },
    prelude::*,
    ptr::OwningPtr,
};
use seldom_state::prelude::*;

pub struct Affected<M: 'static, T: IntoResultSystem<In<Entity>, (), M>>(
    pub T::System,
    PhantomData<fn(M)>,
);

impl<M: 'static, T: IntoResultSystem<In<Entity>, (), M>> Affected<M, T> {
    pub fn by(effect: T) -> Self {
        Self(T::into_result_system(effect), PhantomData)
    }
}

impl<M: 'static, T: IntoResultSystem<In<Entity>, (), M>> DynamicBundle for Affected<M, T> {
    type Effect = Self;

    fn get_components(self, _: &mut impl FnMut(StorageType, OwningPtr<'_>)) -> Self::Effect {
        self
    }
}

unsafe impl<M: 'static, T: IntoResultSystem<In<Entity>, (), M>> Bundle for Affected<M, T> {
    fn component_ids(_: &mut ComponentsRegistrator, _: &mut impl FnMut(ComponentId)) {}

    fn get_component_ids(_: &Components, _: &mut impl FnMut(Option<ComponentId>)) {}

    fn register_required_components(_: &mut ComponentsRegistrator, _: &mut RequiredComponents) {}
}

impl<M: 'static, T: IntoResultSystem<In<Entity>, (), M>> BundleEffect for Affected<M, T> {
    fn apply(self, entity: &mut EntityWorldMut) {
        let id = entity.id();
        entity
            .world_scope(|world| world.run_system_once_with(self.0, id))
            .expect("Couldn't run system")
            .expect("Failed to apply effect");
    }
}

#[derive(Debug)]
pub struct Observed<
    E: Event,
    B: Bundle,
    M: 'static,
    T: IntoObserverSystem<E, B, M> + 'static + Send + Sync,
>(pub T, PhantomData<fn(E, B, M)>);

impl<E: Event, B: Bundle, M, T: IntoObserverSystem<E, B, M> + Clone + 'static + Send + Sync> Clone
    for Observed<E, B, M, T>
{
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<E: Event, B: Bundle, M, T: IntoObserverSystem<E, B, M> + 'static + Send + Sync>
    Observed<E, B, M, T>
{
    pub fn by(observer: T) -> Self {
        Self(observer, PhantomData)
    }
}

impl<E: Event, B: Bundle, M: 'static, T: IntoObserverSystem<E, B, M> + 'static + Send + Sync>
    DynamicBundle for Observed<E, B, M, T>
{
    type Effect = Self;

    fn get_components(self, _: &mut impl FnMut(StorageType, OwningPtr<'_>)) -> Self::Effect {
        self
    }
}

unsafe impl<E: Event, B: Bundle, M: 'static, T: IntoObserverSystem<E, B, M> + 'static + Send + Sync>
    Bundle for Observed<E, B, M, T>
{
    fn component_ids(_: &mut ComponentsRegistrator, _: &mut impl FnMut(ComponentId)) {}

    fn get_component_ids(_: &Components, _: &mut impl FnMut(Option<ComponentId>)) {}

    fn register_required_components(_: &mut ComponentsRegistrator, _: &mut RequiredComponents) {}
}

impl<E: Event, B: Bundle, M: 'static, T: IntoObserverSystem<E, B, M> + 'static + Send + Sync>
    BundleEffect for Observed<E, B, M, T>
{
    fn apply(self, entity: &mut EntityWorldMut) {
        entity.observe(self.0);
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct WithChild<B: Bundle>(pub B);
impl<B: Bundle> DynamicBundle for WithChild<B> {
    type Effect = Self;

    fn get_components(self, _: &mut impl FnMut(StorageType, OwningPtr<'_>)) -> Self::Effect {
        self
    }
}

unsafe impl<B: Bundle> Bundle for WithChild<B> {
    fn component_ids(_: &mut ComponentsRegistrator, _: &mut impl FnMut(ComponentId)) {}

    fn get_component_ids(_: &Components, _: &mut impl FnMut(Option<ComponentId>)) {}

    fn register_required_components(_: &mut ComponentsRegistrator, _: &mut RequiredComponents) {}
}

impl<B: Bundle> BundleEffect for WithChild<B> {
    fn apply(self, entity: &mut EntityWorldMut) {
        entity.with_child(self.0);
    }
}

pub struct Direct;
pub struct Resulted;

pub trait IntoResultSystem<In: SystemInput, Out: 'static, Marker>: 'static {
    type System: System<In = In, Out = Result<Out>>;

    fn into_result_system(this: Self) -> Self::System;
}

impl<In: SystemInput, Out: 'static, Marker, S: 'static + IntoSystem<In, Out, Marker>>
    IntoResultSystem<In, Out, (Direct, Marker)> for S
{
    type System = ResultSystem<S::System>;

    fn into_result_system(this: Self) -> Self::System {
        ResultSystem(IntoSystem::into_system(this))
    }
}

impl<In: SystemInput, Out: 'static, Marker, S: 'static + IntoSystem<In, Result<Out>, Marker>>
    IntoResultSystem<In, Out, (Resulted, Marker)> for S
{
    type System = S::System;

    fn into_result_system(this: Self) -> Self::System {
        IntoSystem::into_system(this)
    }
}

impl<In: SystemInput, Out: 'static, Marker, S: 'static + IntoSystem<In, Never, Marker>>
    IntoResultSystem<In, Out, (fn() -> Never, Marker)> for S
{
    type System = NeverSystem<S::System, Result<Out>>;

    fn into_result_system(this: Self) -> Self::System {
        NeverSystem(IntoSystem::into_system(this), PhantomData)
    }
}

pub struct ResultSystem<S: System>(S);
impl<S: System> System for ResultSystem<S> {
    type In = S::In;
    type Out = Result<S::Out>;

    fn name(&self) -> Cow<'static, str> {
        self.0.name()
    }

    fn component_access(&self) -> &Access<ComponentId> {
        self.0.component_access()
    }

    fn archetype_component_access(&self) -> &Access<ArchetypeComponentId> {
        self.0.archetype_component_access()
    }

    fn is_send(&self) -> bool {
        self.0.is_send()
    }

    fn is_exclusive(&self) -> bool {
        self.0.is_exclusive()
    }

    fn has_deferred(&self) -> bool {
        self.0.has_deferred()
    }

    unsafe fn run_unsafe(
        &mut self,
        input: SystemIn<'_, Self>,
        world: UnsafeWorldCell,
    ) -> Self::Out {
        Ok(unsafe { self.0.run_unsafe(input, world) })
    }

    fn apply_deferred(&mut self, world: &mut World) {
        self.0.apply_deferred(world);
    }

    fn queue_deferred(&mut self, world: DeferredWorld) {
        self.0.queue_deferred(world);
    }

    unsafe fn validate_param_unsafe(
        &mut self,
        world: UnsafeWorldCell,
    ) -> Result<(), SystemParamValidationError> {
        unsafe { self.0.validate_param_unsafe(world) }
    }

    fn initialize(&mut self, world: &mut World) {
        self.0.initialize(world);
    }

    fn update_archetype_component_access(&mut self, world: UnsafeWorldCell) {
        self.0.update_archetype_component_access(world);
    }

    fn check_change_tick(&mut self, change_tick: Tick) {
        self.0.check_change_tick(change_tick);
    }

    fn get_last_run(&self) -> Tick {
        self.0.get_last_run()
    }

    fn set_last_run(&mut self, last_run: Tick) {
        self.0.set_last_run(last_run);
    }
}

pub struct NeverSystem<S: System<Out = Never>, Out: 'static>(S, PhantomData<fn() -> Out>);
impl<S: System<Out = Never>, Out: 'static> System for NeverSystem<S, Out> {
    type In = S::In;
    type Out = Out;

    fn name(&self) -> Cow<'static, str> {
        self.0.name()
    }

    fn component_access(&self) -> &Access<ComponentId> {
        self.0.component_access()
    }

    fn archetype_component_access(&self) -> &Access<ArchetypeComponentId> {
        self.0.archetype_component_access()
    }

    fn is_send(&self) -> bool {
        self.0.is_send()
    }

    fn is_exclusive(&self) -> bool {
        self.0.is_exclusive()
    }

    fn has_deferred(&self) -> bool {
        self.0.has_deferred()
    }

    unsafe fn run_unsafe(
        &mut self,
        input: SystemIn<'_, Self>,
        world: UnsafeWorldCell,
    ) -> Self::Out {
        unsafe { self.0.run_unsafe(input, world) }
    }

    fn apply_deferred(&mut self, world: &mut World) {
        self.0.apply_deferred(world);
    }

    fn queue_deferred(&mut self, world: DeferredWorld) {
        self.0.queue_deferred(world);
    }

    unsafe fn validate_param_unsafe(
        &mut self,
        world: UnsafeWorldCell,
    ) -> Result<(), SystemParamValidationError> {
        unsafe { self.0.validate_param_unsafe(world) }
    }

    fn initialize(&mut self, world: &mut World) {
        self.0.initialize(world);
    }

    fn update_archetype_component_access(&mut self, world: UnsafeWorldCell) {
        self.0.update_archetype_component_access(world);
    }

    fn check_change_tick(&mut self, change_tick: Tick) {
        self.0.check_change_tick(change_tick);
    }

    fn get_last_run(&self) -> Tick {
        self.0.get_last_run()
    }

    fn set_last_run(&mut self, last_run: Tick) {
        self.0.set_last_run(last_run);
    }
}

pub fn wait(
    duration: Duration,
) -> impl Fn(Res<Time>, Local<Option<Duration>>) -> bool + 'static + Send + Sync {
    wait_on::<()>(duration)
}

pub fn wait_on<Ctx: 'static + Send + Sync + Default>(
    duration: Duration,
) -> impl Fn(Res<Time<Ctx>>, Local<Option<Duration>>) -> bool + 'static + Send + Sync {
    move |time: Res<Time<Ctx>>, mut started: Local<Option<Duration>>| -> bool {
        let now = time.elapsed();
        let prev = *started.get_or_insert(now);
        now - prev >= duration
    }
}

pub fn trans_wait(duration: Duration) -> impl EntityTrigger<Out = bool> {
    trans_wait_on::<()>(duration)
}

pub fn trans_wait_on<Ctx: 'static + Send + Sync + Default>(
    duration: Duration,
) -> impl EntityTrigger<Out = bool> {
    (move |_: In<Entity>, time: Res<Time<Ctx>>, mut started: Local<Option<Duration>>| {
        let now = time.elapsed();
        let prev = *started.get_or_insert(now);
        now - prev >= duration
    })
    .into_trigger()
}

pub fn despawn_recursive_if<S: RelationshipTarget, F: QueryFilter>(e: EntityWorldMut) {
    fn inner<S: RelationshipTarget, F: QueryFilter>(
        world: &mut World,
        entity: Entity,
        query: &QueryState<(), F>,
    ) {
        let related = world
            .get::<S>(entity)
            .into_iter()
            .flat_map(S::iter)
            .collect::<Box<[_]>>();

        if query.get_manual(world, entity).is_ok() {
            world.despawn(entity);
        }

        for child in related {
            inner::<S, F>(world, child, query);
        }
    }

    let id = e.id();
    let world = e.into_world_mut();

    let query = QueryState::<(), F>::new(world);
    inner::<S, F>(world, id, &query);
}

pub fn insert_recursive_if<S: RelationshipTarget, F: QueryFilter, B: Bundle>(
    mut bundle: impl FnMut(Entity) -> B + 'static + Send + Sync,
) -> impl EntityCommand {
    fn inner<S: RelationshipTarget, F: QueryFilter, B: Bundle>(
        bundle: &mut (impl FnMut(Entity) -> B + 'static + Send + Sync),
        world: &mut World,
        entity: Entity,
        query: &QueryState<(), F>,
    ) {
        let related = world
            .get::<S>(entity)
            .into_iter()
            .flat_map(S::iter)
            .collect::<Box<[_]>>();

        if query.get_manual(world, entity).is_ok() {
            world.entity_mut(entity).insert(bundle(entity));
        }

        for child in related {
            inner::<S, F, B>(bundle, world, child, query);
        }
    }

    move |e: EntityWorldMut| {
        let id = e.id();
        let world = e.into_world_mut();

        let query = QueryState::<(), F>::new(world);
        inner::<S, F, B>(&mut bundle, world, id, &query);
    }
}

pub fn suspend(mut e: EntityWorldMut) {
    e.insert_recursive::<Children>(Disabled);
}

pub fn resume(mut e: EntityWorldMut) {
    // I don't know why, but I *have* to do this otherwise observers break.
    if let Some(Disabled) = e.take::<Disabled>() {
        e.remove_recursive::<Children, Disabled>();
        let trns = e.take::<(Transform, GlobalTransform)>();
        let physics = e.take::<(RigidBody, Collider)>();

        e.remove_with_requires::<(RigidBody, Collider, Transform)>();
        match (trns, physics) {
            (None, None) => {}
            (None, Some(physics)) => {
                e.insert(physics);
            }
            (Some(trns), None) => {
                e.insert(trns);
            }
            (Some(trns), Some(physics)) => {
                e.insert((trns, physics));
            }
        }
    }
}
