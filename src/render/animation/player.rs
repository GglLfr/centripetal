use crate::{
    math::{GlobalTransform2d, Transform2d},
    prelude::*,
    render::{
        animation::{AnimationDirection, AnimationFrame, AnimationIndices, AnimationSheet},
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

#[derive(Component, Debug, Default, Clone, Copy)]
struct AnimationState {
    index: usize,
    time: Duration,
    ticked: bool,
}

#[derive(QueryData)]
#[query_data(mutable, derive(Debug))]
pub struct AnimationQuery {
    pub animation: Read<Animation>,
    pub tag: Ref<'static, AnimationTag>,
    state: Write<AnimationState>,
}

impl AnimationQueryItem<'_, '_> {
    pub fn assets<'a>(&self, sheets: &'a Assets<AnimationSheet>) -> Option<AnimationAssets<'a>> {
        let sheet = sheets.get(self.animation.id())?;
        let frame = sheet.frames.get(self.state.index)?;
        let frame_tag = sheet.frame_tags.get(self.tag.as_str())?;

        Some(AnimationAssets { sheet, frame, frame_tag })
    }

    pub fn is_ticked(&self) -> bool {
        self.state.ticked
    }
}

impl AnimationQueryReadOnlyItem<'_, '_> {
    pub fn assets<'a>(&self, sheets: &'a Assets<AnimationSheet>) -> Option<AnimationAssets<'a>> {
        let sheet = sheets.get(self.animation.id())?;
        let frame = sheet.frames.get(self.state.index)?;
        let frame_tag = sheet.frame_tags.get(self.tag.as_str())?;

        Some(AnimationAssets { sheet, frame, frame_tag })
    }

    pub fn is_ticked(&self) -> bool {
        self.state.ticked
    }
}

pub struct AnimationAssets<'a> {
    pub sheet: &'a AnimationSheet,
    pub frame: &'a AnimationFrame,
    pub frame_tag: &'a AnimationIndices,
}

#[derive(Message, EntityEvent, Debug, Clone)]
pub struct AnimateEnd {
    pub entity: Entity,
    pub tag: Cow<'static, str>,
}

#[derive(Message, EntityEvent, Debug, Clone)]
pub struct AnimateLoop {
    pub entity: Entity,
    pub tag: Cow<'static, str>,
}

fn update_animation_states(
    commands: ParallelCommands,
    mut end_writer: MessageWriter<AnimateEnd>,
    mut loop_writer: MessageWriter<AnimateLoop>,
    time: Res<Time>,
    sheets: Res<Assets<AnimationSheet>>,
    sheet_changes: Query<(), Or<(Changed<Animation>, AssetChanged<Animation>)>>,
    transitions: Query<&AnimationTransition>,
    states: Query<(Entity, AnimationQuery, &AnimationRepeat, &AnimationEvents)>,
    mut events: Local<Parallel<[(Vec<AnimateEnd>, Vec<AnimateLoop>); 2]>>,
) {
    let dt = time.delta();
    states.par_iter_inner().for_each_init(
        || events.borrow_local_mut(),
        |events, (entity, anim_query, &repeat, &event_listener)| {
            let [(pull_end, pull_loop), (push_end, push_loop)] = &mut **events;

            let Some(sheet) = sheets.get(anim_query.animation.id()) else { return };
            let Some(frame_tag) = sheet.frame_tags.get(anim_query.tag.as_str()) else { return };

            let state = anim_query.state.into_inner();
            state.ticked = false;

            let (first, last, incr) = match frame_tag.direction {
                AnimationDirection::Forward => (*frame_tag.indices.start(), *frame_tag.indices.end(), 1),
                AnimationDirection::Reverse => (*frame_tag.indices.end(), *frame_tag.indices.start(), -1),
            };

            // If the asset or the tag changed, reset the frame index.
            if anim_query.tag.is_changed() || sheet_changes.contains(entity) {
                let transition = transitions.get(entity).copied();
                if transition.is_ok() {
                    commands.command_scope(|mut commands| {
                        commands.entity(entity).try_remove::<AnimationTransition>();
                    });
                }

                state.ticked = true;
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
                state.ticked = match repeat {
                    AnimationRepeat::Halt => {
                        if state.index == last {
                            return
                        } else {
                            state.index = state.index.wrapping_add_signed(incr);
                            if state.index == last {
                                event_listener.emit(
                                    AnimateEnd {
                                        entity,
                                        tag: anim_query.tag.clone(),
                                    },
                                    pull_end,
                                    push_end,
                                );
                            }
                            state.time = new_time;
                            true
                        }
                    }
                    AnimationRepeat::Loop => {
                        state.index = if state.index == last {
                            event_listener.emit(
                                AnimateLoop {
                                    entity,
                                    tag: anim_query.tag.clone(),
                                },
                                pull_loop,
                                push_loop,
                            );
                            first
                        } else {
                            state.index.wrapping_add_signed(incr)
                        };
                        state.time = new_time;
                        true
                    }
                }
            }

            // `dt` is added at the end so the first frame has some time to show up in the render world.
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

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnimationSystems {
    Update,
    Updated,
    Draw,
}

pub(super) fn plugin(app: &mut App) {
    app.configure_sets(
        PostUpdate,
        (
            (AnimationSystems::Update, AnimationSystems::Updated)
                .chain()
                .before(TransformSystems::Propagate),
            AnimationSystems::Draw.after(TransformSystems::Propagate),
        ),
    )
    .add_message::<AnimateEnd>()
    .add_message::<AnimateLoop>()
    .add_systems(
        PostUpdate,
        (
            update_animation_states.in_set(AnimationSystems::Update),
            draw_animations.in_set(AnimationSystems::Draw),
        ),
    );
}
