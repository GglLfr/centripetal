use std::{borrow::Cow, ops::Range, time::Duration};

use bevy::{
    ecs::{
        bundle::{BundleEffect, DynamicBundle},
        component::{ComponentId, Components, ComponentsRegistrator, HookContext, RequiredComponents, StorageType},
        system::{BoxedSystem, IntoPipeSystem, RunSystemError},
        world::DeferredWorld,
    },
    prelude::*,
    ptr::OwningPtr,
};

use crate::{
    Sprites,
    graphics::{EntityColor, SpriteDrawer, SpriteSection, SpriteSheet},
};

#[derive(Debug)]
pub struct AnimationFrom(BoxedSystem<In<Entity>, (Handle<SpriteSheet>, String)>);
impl AnimationFrom {
    pub fn new<M, S: 'static + Into<String>>(system: impl IntoSystem<In<Entity>, (Handle<SpriteSheet>, S), M>) -> Self {
        Self(Box::new(IntoPipeSystem::into_system(system.pipe(
            |In((handle, string)): In<(Handle<SpriteSheet>, S)>| (handle, string.into()),
        ))))
    }

    pub fn sprite<S: 'static + Into<String>>(
        provider: impl FnOnce(&Sprites) -> (Handle<SpriteSheet>, S) + 'static + Send + Sync,
    ) -> Self {
        let mut provider = Some(provider);
        Self::new(move |_: In<Entity>, sprites: Res<Sprites>| {
            provider.take().expect("This system must only be run once")(&sprites)
        })
    }
}

impl DynamicBundle for AnimationFrom {
    type Effect = Self;

    fn get_components(self, func: &mut impl FnMut(StorageType, OwningPtr<'_>)) -> Self::Effect {
        Animation::default().get_components(func);
        self
    }
}

unsafe impl Bundle for AnimationFrom {
    fn component_ids(components: &mut ComponentsRegistrator, ids: &mut impl FnMut(ComponentId)) {
        Animation::component_ids(components, ids);
    }

    fn get_component_ids(components: &Components, ids: &mut impl FnMut(Option<ComponentId>)) {
        Animation::get_component_ids(components, ids);
    }

    fn register_required_components(components: &mut ComponentsRegistrator, required_components: &mut RequiredComponents) {
        <Animation as Bundle>::register_required_components(components, required_components);
    }
}

impl BundleEffect for AnimationFrom {
    fn apply(mut self, entity: &mut EntityWorldMut) {
        let id = entity.id();
        let (sprite, key) = entity.world_scope(|world| {
            self.0.initialize(world);
            self.0
                .validate_param(world)
                .map_err(|err| RunSystemError::InvalidParams {
                    system: self.0.name(),
                    err,
                })
                .expect("Couldn't run system");
            self.0.run(id, world)
        });

        entity
            .get_mut::<Animation>()
            .expect("`Animation` was erroneously removed")
            .sprite = sprite;

        entity.world_scope(|world| {
            world.commands().entity(id).queue(AnimateKey {
                key: key.into(),
                reset_duration: true,
                fire_exit: false,
            });
        });
    }
}

#[derive(Debug, Clone, Default, Component)]
#[require(SpriteDrawer, AnimationData, AnimationMode)]
#[component(on_insert = on_animation_insert)]
pub struct Animation {
    pub sprite: Handle<SpriteSheet>,
    key: String,
}

impl Animation {
    pub fn new(sprite: Handle<SpriteSheet>, key: impl Into<String>) -> Self {
        Self { sprite, key: key.into() }
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

#[derive(Debug, Copy, Clone, Default, Component)]
pub enum AnimationMode {
    #[default]
    Finish,
    Saturate,
    Repeat,
}

#[derive(Debug, Copy, Clone, Default, Component)]
pub struct AnimationData {
    pub time: Duration,
    pub frame: usize,
    pub finished: bool,
}

#[derive(Debug, Clone)]
pub struct AnimateKey {
    pub key: Cow<'static, str>,
    reset_duration: bool,
    fire_exit: bool,
}

impl AnimateKey {
    pub fn new(key: impl Into<Cow<'static, str>>, reset_duration: bool) -> Self {
        Self {
            key: key.into(),
            reset_duration,
            fire_exit: true,
        }
    }

    pub fn continuous(key: impl Into<Cow<'static, str>>) -> Self {
        Self::new(key, false)
    }

    pub fn reset(key: impl Into<Cow<'static, str>>) -> Self {
        Self::new(key, true)
    }
}

impl EntityCommand<Result> for AnimateKey {
    fn apply(self, mut entity: EntityWorldMut) -> Result {
        let id = entity.id();
        if self.fire_exit {
            let key = std::mem::take(&mut entity.get_mut::<Animation>().ok_or("`Animation` not found")?.key);
            entity.trigger(OnAnimateExit(key));
        }

        let start = entity.world_scope(|world| {
            world
                .get::<Animation>(id)
                .map(|anim| anim.sprite.id())
                .and_then(|id| world.get_resource::<Assets<SpriteSheet>>()?.get(id))
                .and_then(|sprite| sprite.tags.get(&*self.key))
                .map(|range| range.start)
        });

        if let Some(mut anim) = entity.get_mut::<Animation>() {
            (*self.key).clone_into(&mut anim.key);
            if let Some(start) = start &&
                let Some(mut data) = entity.get_mut::<AnimationData>()
            {
                if self.reset_duration {
                    data.time = Duration::ZERO;
                }
                data.frame = start;
                data.finished = false;

                entity.trigger(OnAnimateEnter(self.key.into()));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Event, Deref)]
pub struct OnAnimateEnter(String);
#[derive(Debug, Clone, Event, Deref)]
pub struct OnAnimateDone(String);
#[derive(Debug, Clone, Event, Deref)]
pub struct OnAnimateExit(String);

fn on_animation_insert(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    let key = std::mem::take(&mut world.entity_mut(entity).get_mut::<Animation>().unwrap().key);
    world.commands().entity(entity).queue(AnimateKey {
        key: key.into(),
        reset_duration: true,
        fire_exit: true,
    });
}

pub fn update_animations(
    commands: ParallelCommands,
    time: Res<Time>,
    sprite_sheets: Res<Assets<SpriteSheet>>,
    mut animations: Query<(Entity, &Animation, &AnimationMode, &mut AnimationData)>,
) {
    let delta = time.delta();
    animations.par_iter_mut().for_each(|(e, animation, &mode, mut data)| {
        let Some(sprite) = sprite_sheets.get(&animation.sprite) else { return };

        data.time += delta;
        if let Some(Range { start, end }) = sprite.tags.get(&animation.key).cloned() {
            data.frame = data.frame.clamp(start, end);
            while data.time > Duration::ZERO {
                if let Some(&duration) = sprite.durations.get(data.frame) &&
                    data.time >= duration
                {
                    if data.frame < end {
                        data.frame += 1;
                        data.time -= duration;
                    } else if matches!(mode, AnimationMode::Repeat) {
                        data.frame = start;
                        data.time -= duration;
                    } else {
                        break
                    }
                } else {
                    break
                }
            }

            if !matches!(mode, AnimationMode::Repeat) &&
                data.frame == end &&
                let Some(&duration) = sprite.durations.get(end) &&
                data.time >= duration &&
                !std::mem::replace(&mut data.finished, true)
            {
                data.time -= duration;
                commands.command_scope(|mut commands| {
                    commands.entity(e).trigger(OnAnimateDone(animation.key.clone()));
                });
            }
        }
    });
}

pub fn draw_animations(
    sprite_sheets: Res<Assets<SpriteSheet>>,
    sprites: Res<Assets<SpriteSection>>,
    animations: Query<(
        &Animation,
        &AnimationData,
        &AnimationMode,
        &SpriteDrawer,
        Option<&EntityColor>,
    )>,
) {
    animations.par_iter().for_each(|(animation, data, &mode, drawer, color)| {
        let Some(frame) = sprite_sheets
            .get(&animation.sprite)
            .and_then(|sheet| sheet.frames.get(data.frame))
            .and_then(|frame| sprites.get(frame))
        else {
            return
        };

        if !data.finished || !matches!(mode, AnimationMode::Finish) {
            drawer.draw_at(
                Vec3::ZERO,
                Rot2::IDENTITY,
                frame.sprite_with(color.copied().unwrap_or_default().0, None, default()),
            );
        }
    });
}
