use crate::{
    math::{GlobalTransform2d, Transform2d},
    prelude::*,
    render::{
        animation::{AnimationDirection, AnimationSheet},
        painter::{Painter, PainterParam},
    },
};

#[derive(Reflect, Component, Debug, Deref, DerefMut)]
#[require(AnimationRepeat, AnimationEvents, AnimationState, Painter, Transform2d)]
#[reflect(Component, Debug)]
pub struct Animation {
    pub handle: Handle<AnimationSheet>,
}

impl AsAssetId for Animation {
    type Asset = AnimationSheet;

    fn as_asset_id(&self) -> AssetId<Self::Asset> {
        self.handle.id()
    }
}

impl From<Handle<AnimationSheet>> for Animation {
    fn from(value: Handle<AnimationSheet>) -> Self {
        Self { handle: value.into() }
    }
}

impl From<&Handle<AnimationSheet>> for Animation {
    fn from(value: &Handle<AnimationSheet>) -> Self {
        Self { handle: value.clone() }
    }
}

#[derive(Reflect, Component, Debug, Deref, DerefMut)]
#[component(immutable)]
#[reflect(Component, Debug)]
pub struct AnimationTag(pub Cow<'static, str>);
impl AnimationTag {
    pub fn new(value: impl Into<Cow<'static, str>>) -> Self {
        Self(value.into())
    }

    // BLOCK: When `str_as_str` (#130366) is stabilized, remove this method.
    pub const fn as_str(&self) -> &str {
        match self.0 {
            Cow::Borrowed(borrowed) => borrowed,
            Cow::Owned(ref owned) => owned.as_str(),
        }
    }
}

#[derive(Reflect, Component, Debug, Default, Clone, Copy)]
#[component(immutable, storage = "SparseSet")]
#[reflect(Component, Debug, Default, FromWorld, Clone)]
pub enum AnimationTransition {
    #[default]
    Discrete,
    Continuous,
}

impl<T: Into<Cow<'static, str>>> From<T> for AnimationTag {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

#[derive(Reflect, Component, Debug, Default, Clone, Copy)]
#[reflect(Component, Debug, Default, FromWorld, Clone)]
pub enum AnimationRepeat {
    #[default]
    Halt,
    Loop,
}

#[derive(Reflect, Component, Debug, Default, Clone, Copy)]
#[reflect(Component, Debug, Default, FromWorld, Clone)]
pub enum AnimationEvents {
    #[default]
    None,
    Message,
    Observer,
}

impl AnimationEvents {
    pub fn emit<T>(self, value: T, pull: &mut impl Extend<T>, push: &mut impl Extend<T>) {
        // BLOCK: When `extend_one` (#72631) is stabilized, use that instead.
        match self {
            Self::None => {}
            Self::Message => pull.extend([value]),
            Self::Observer => push.extend([value]),
        }
    }
}

#[derive(Message, EntityEvent, Debug, Clone)]
pub struct AnimationEnd {
    pub entity: Entity,
    pub tag: Cow<'static, str>,
}

#[derive(Message, EntityEvent, Debug, Clone)]
pub struct AnimationLoop {
    pub entity: Entity,
    pub tag: Cow<'static, str>,
}

#[derive(Component, Debug, Default, Clone, Copy)]
struct AnimationState {
    index: usize,
    time: Duration,
}

fn update_animation_states(
    commands: ParallelCommands,
    mut end_writer: MessageWriter<AnimationEnd>,
    mut loop_writer: MessageWriter<AnimationLoop>,
    time: Res<Time>,
    sheets: Res<Assets<AnimationSheet>>,
    sheet_changes: Query<(), Or<(Changed<Animation>, AssetChanged<Animation>)>>,
    transitions: Query<&AnimationTransition>,
    states: Query<(
        Entity,
        &Animation,
        Ref<AnimationTag>,
        &mut AnimationState,
        &AnimationRepeat,
        &AnimationEvents,
    )>,
    mut events: Local<Parallel<[(Vec<AnimationEnd>, Vec<AnimationLoop>); 2]>>,
) {
    let dt = time.delta();
    states.par_iter_inner().for_each_init(
        || events.borrow_local_mut(),
        |events, (entity, anim, tag, state, &repeat, &event_listener)| {
            let [(pull_end, pull_loop), (push_end, push_loop)] = &mut **events;

            let Some(sheet) = sheets.get(anim.id()) else { return };
            let Some(frame_tag) = sheet.frame_tags.get(tag.as_str()) else { return };
            let state = state.into_inner();

            let (first, last, incr) = match frame_tag.direction {
                AnimationDirection::Forward => (*frame_tag.indices.start(), *frame_tag.indices.end(), 1),
                AnimationDirection::Reverse => (*frame_tag.indices.end(), *frame_tag.indices.start(), -1),
            };

            // If the asset or the tag changed, reset the frame index.
            if tag.is_changed() || sheet_changes.contains(entity) {
                let transition = transitions.get(entity).copied();
                if transition.is_ok() {
                    commands.command_scope(|mut commands| {
                        commands.entity(entity).try_remove::<AnimationTransition>();
                    });
                }

                state.index = first;
                state.time = match transition.unwrap_or_default() {
                    AnimationTransition::Discrete => Duration::ZERO,
                    AnimationTransition::Continuous => state.time,
                };
            }

            loop {
                // `break` : Stop propagating frames and finally add `dt` to the accumulated frame time.
                // `return`: Frame time shouldn't be accumulated any longer.
                let Some(frame) = sheet.frames.get(state.index) else { return };
                let Some(new_time) = state.time.checked_sub(frame.duration) else { break };
                match repeat {
                    AnimationRepeat::Halt => {
                        if state.index == last {
                            state.time = frame.duration;
                            return
                        } else {
                            state.index = state.index.wrapping_add_signed(incr);
                            if state.index == last {
                                event_listener.emit(AnimationEnd { entity, tag: tag.clone() }, pull_end, push_end);
                            }
                            state.time = new_time;
                        }
                    }
                    AnimationRepeat::Loop => {
                        state.index = if state.index == last {
                            event_listener.emit(AnimationLoop { entity, tag: tag.clone() }, pull_loop, push_loop);
                            first
                        } else {
                            state.index.wrapping_add_signed(incr)
                        };
                        state.time = new_time;
                    }
                }
            }

            // `dt` is added at the end so the first frame has time to show up in the render world.
            state.time += dt;
        },
    );

    for [(pull_end, pull_loop), (push_end, push_loop)] in events.iter_mut() {
        end_writer.write_batch(pull_end.drain(..));
        loop_writer.write_batch(pull_loop.drain(..));
        commands.command_scope(|mut commands| {
            for event in push_end.drain(..) {
                commands.trigger(event);
            }
            for event in push_loop.drain(..) {
                commands.trigger(event);
            }
        });
    }
}

fn draw_animations(
    param: PainterParam,
    sheets: Res<Assets<AnimationSheet>>,
    animations: Query<(&Animation, &AnimationState, &Painter, &GlobalTransform2d)>,
) {
    animations.par_iter_inner().for_each(|(anim, &state, painter, &trns)| {
        let Some(sheet) = sheets.get(anim.id()) else { return };
        let Some(frame) = sheet.frames.get(state.index) else { return };

        let mut ctx = param.ctx(painter);
        ctx.layer = trns.z;
        ctx.rect(&frame.region, trns.affine * Affine2::from_translation(frame.offset), default());
    });
}

pub(super) fn plugin(app: &mut App) {
    app.add_message::<AnimationEnd>().add_message::<AnimationLoop>().add_systems(
        PostUpdate,
        (update_animation_states, draw_animations.after(TransformSystems::Propagate)).chain(),
    );
}
