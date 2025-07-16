use std::{marker::PhantomData, time::Duration};

use bevy::{
    ecs::{
        bundle::{BundleEffect, DynamicBundle},
        component::{ComponentId, Components, ComponentsRegistrator, RequiredComponents, StorageType},
        system::IntoObserverSystem,
    },
    prelude::*,
    ptr::OwningPtr,
};
use seldom_state::prelude::*;

pub struct Observe<E: Event, B: Bundle, M: 'static, T: IntoObserverSystem<E, B, M> + Sync>(pub T, PhantomData<fn(E, B, M)>);
impl<E: Event, B: Bundle, M, T: IntoObserverSystem<E, B, M> + Sync> Observe<E, B, M, T> {
    pub fn by(observer: T) -> Self {
        Self(observer, PhantomData)
    }
}

impl<E: Event, B: Bundle, M: 'static, T: IntoObserverSystem<E, B, M> + Sync> DynamicBundle for Observe<E, B, M, T> {
    type Effect = Self;

    fn get_components(self, _: &mut impl FnMut(StorageType, OwningPtr<'_>)) -> Self::Effect {
        self
    }
}

unsafe impl<E: Event, B: Bundle, M: 'static, T: IntoObserverSystem<E, B, M> + Sync> Bundle for Observe<E, B, M, T> {
    fn component_ids(_: &mut ComponentsRegistrator, _: &mut impl FnMut(ComponentId)) {}

    fn get_component_ids(_: &Components, _: &mut impl FnMut(Option<ComponentId>)) {}

    fn register_required_components(_: &mut ComponentsRegistrator, _: &mut RequiredComponents) {}
}

impl<E: Event, B: Bundle, M: 'static, T: IntoObserverSystem<E, B, M> + Sync> BundleEffect for Observe<E, B, M, T> {
    fn apply(self, entity: &mut EntityWorldMut) {
        entity.observe(self.0);
    }
}

pub fn wait(duration: Duration) -> impl Fn(Res<Time>, Local<Option<Duration>>) -> bool + 'static + Send + Sync {
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

pub fn trans_wait_on<Ctx: 'static + Send + Sync + Default>(duration: Duration) -> impl EntityTrigger<Out = bool> {
    (move |_: In<Entity>, time: Res<Time<Ctx>>, mut started: Local<Option<Duration>>| {
        let now = time.elapsed();
        let prev = *started.get_or_insert(now);
        now - prev >= duration
    })
    .into_trigger()
}
