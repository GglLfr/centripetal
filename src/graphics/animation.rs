use std::{borrow::Cow, ops::Range, time::Duration};

use bevy::{
    ecs::{component::HookContext, world::DeferredWorld},
    prelude::*,
};

use crate::graphics::{EntityColor, SpriteDrawer, SpriteSection, SpriteSheet};

#[derive(Debug, Clone, Component)]
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
}

#[derive(Debug, Copy, Clone, Default, Component)]
pub struct AnimationData {
    pub time: Duration,
    pub frame: usize,
    pub finished: bool,
}

#[derive(Debug, Clone)]
pub struct AnimateKey(pub Cow<'static, str>, bool);
impl AnimateKey {
    pub fn new(key: impl Into<Cow<'static, str>>) -> Self {
        Self(key.into(), false)
    }
}

impl EntityCommand<Result> for AnimateKey {
    fn apply(self, mut entity: EntityWorldMut) -> Result {
        let id = entity.id();
        if !self.1 {
            let key = std::mem::take(&mut entity.get_mut::<Animation>().ok_or("`Animation` not found")?.key);
            entity.trigger(OnAnimateExit(key));
        }

        let start = entity.world_scope(|world| {
            world
                .get::<Animation>(id)
                .map(|anim| anim.sprite.id())
                .and_then(|id| world.get_resource::<Assets<SpriteSheet>>()?.get(id))
                .and_then(|sprite| sprite.tags.get(&*self.0))
                .map(|range| range.start)
        });

        if let Some(mut anim) = entity.get_mut::<Animation>() {
            (*self.0).clone_into(&mut anim.key);
            if let Some(start) = start {
                entity.insert(AnimationData {
                    time: Duration::ZERO,
                    frame: start,
                    finished: false,
                });
            }

            entity.trigger(OnAnimateEnter(self.0.into()));
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
    world.commands().entity(entity).queue(AnimateKey(key.into(), true));
}

pub fn update_animations(
    commands: ParallelCommands,
    time: Res<Time>,
    sprite_sheets: Res<Assets<SpriteSheet>>,
    mut animations: Query<(Entity, &Animation, &mut AnimationData)>,
) {
    let delta = time.delta();
    animations.par_iter_mut().for_each(|(e, animation, mut data)| {
        let Some(sprite) = sprite_sheets.get(&animation.sprite) else { return };

        data.time += delta;
        if let Some(Range { start, end }) = sprite.tags.get(&animation.key).cloned() {
            data.frame = data.frame.clamp(start, end);
            while data.time > Duration::ZERO {
                if data.frame < end &&
                    let Some(&duration) = sprite.durations.get(data.frame) &&
                    data.time >= duration
                {
                    data.time -= duration;
                    data.frame += 1;
                } else {
                    break
                }
            }

            if data.frame == end &&
                sprite.durations.get(end).is_some_and(|&duration| data.time >= duration) &&
                !std::mem::replace(&mut data.finished, true)
            {
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
    animations: Query<(&Animation, &AnimationData, &SpriteDrawer, Option<&EntityColor>)>,
) {
    animations.par_iter().for_each(|(animation, data, drawer, color)| {
        let Some(frame) = sprite_sheets
            .get(&animation.sprite)
            .and_then(|sheet| sheet.frames.get(data.frame))
            .and_then(|frame| sprites.get(frame))
        else {
            return
        };

        drawer.draw_at(Vec3::ZERO, Rot2::IDENTITY, None, Sprite {
            color: color.copied().unwrap_or_default().0,
            ..frame.sprite()
        });
    });
}
