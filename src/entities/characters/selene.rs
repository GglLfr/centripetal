use crate::{
    CharacterTextures, MiscTextures,
    control::{GroundControl, GroundControlDirection, GroundControlState, GroundControlStatePrevious, GroundJump, GroundMove, Jump, Movement},
    entities::Hair,
    math::{GlobalTransform2d, Transform2d},
    prelude::*,
    render::{
        CameraTarget, MAIN_LAYER,
        animation::{
            Animation, AnimationEvents, AnimationQueryReadOnly, AnimationRepeat, AnimationSheet, AnimationSystems, AnimationTag, AnimationTransition,
        },
        painter::{Painter, PainterParam},
    },
    world::{EntityCreate, LevelSystems, MessageReaderEntityExt},
};

#[derive(Component, Debug, Clone, Copy)]
pub struct Selene {
    pub hair: Entity,
}

impl Selene {
    pub const IDENT: &'static str = "selene";

    pub const IDLE: &'static str = "idle";
    pub const RUN_LEFT: &'static str = "run_left";
    pub const RUN_TRANS_LEFT: &'static str = "run_trans_left";
    pub const RUN_RIGHT: &'static str = "run_right";
    pub const RUN_TRANS_RIGHT: &'static str = "run_trans_right";
}

#[derive(Component, Debug, Clone)]
#[require(Painter, Transform2d)]
pub struct SeleneHair {
    pub color: LinearRgba,
    pub widths: Vec<f32>,
}

fn spawn_selene(mut commands: Commands, mut messages: MessageReader<EntityCreate>, textures: Res<CharacterTextures>) {
    for &EntityCreate { entity, bounds, .. } in messages.created(Selene::IDENT) {
        let sprite_center = bounds.center();
        let collider_center = vec2(sprite_center.x, bounds.min.y + 12.);

        let hair = commands.spawn_empty().id();
        let hair_back = vec![7., 5.5, 5.5, 4.5, 3.75, 3., 2.5, 2., 1.5, 1.];
        commands.entity(entity).insert((
            Selene { hair },
            // Transforms.
            (
                Transform2d::from_translation(sprite_center.extend(1.)),
                TransformInterpolation,
                CameraTarget::default(),
            ),
            // Rendering.
            (Animation::from(&textures.selene), AnimationTag::new(Selene::IDLE)),
            MAIN_LAYER,
            // Physics.
            (
                SweptCcd::LINEAR,
                Collider::compound(vec![(collider_center - sprite_center, 0., Collider::rectangle(6., 24.))]),
                #[cfg(feature = "dev")]
                DebugRender::none(),
            ),
            // Inputs.
            (
                GroundControl {
                    contact_shape: Collider::compound(vec![(collider_center - sprite_center, 0., Collider::rectangle(4., 22.))]),
                    contact_distance: 1.,
                },
                GroundMove::default(),
                GroundJump::default(),
                actions!(GroundControl[(
                    Action::<Movement>::new(),
                    Down::new(0.5),
                    Bindings::spawn(Cardinal::arrows()),
                ), (
                    Action::<Jump>::new(),
                    bindings![KeyCode::KeyZ],
                )]),
            ),
        ));

        commands
            .entity(hair)
            .insert((ChildOf(entity), Hair::new(hair_back[1..].iter().map(|rad| rad / 3.), 0.2), SeleneHair {
                color: Srgba::hex("70A3C4").unwrap().into(),
                widths: hair_back,
            }));
    }
}

fn react_selene_animations(
    mut commands: Commands,
    states: Query<
        (
            Entity,
            &AnimationTag,
            &AnimationEvents,
            &mut Transform2d,
            &GroundControlStatePrevious,
            Ref<GroundControlState>,
            Ref<GroundControlDirection>,
        ),
        With<Selene>,
    >,
) {
    use AnimationRepeat::*;
    use AnimationTransition::*;
    use GroundControlState::*;

    for (entity, tag, &events, trns, &state_prev, state, dir) in states {
        let mut entity_commands = commands.entity(entity);
        if events & (AnimationEvents::JUST_HALTED | AnimationEvents::JUST_LOOPED) != AnimationEvents::empty() {
            match (tag.as_str(), *state) {
                // a.) Continuation of 1.), proceed to the actual run animation.
                (Selene::RUN_TRANS_LEFT, Run { decelerating: false }) => {
                    entity_commands.insert((AnimationTag::new(Selene::RUN_LEFT), Loop, Continuous));
                }
                (Selene::RUN_TRANS_RIGHT, Run { decelerating: false }) => {
                    entity_commands.insert((AnimationTag::new(Selene::RUN_RIGHT), Loop, Continuous));
                }
                // b.) Continuation of 2.), finish with the idle animation.
                (Selene::RUN_TRANS_LEFT | Selene::RUN_TRANS_RIGHT, Idle) => {
                    entity_commands.insert((AnimationTag::new(Selene::IDLE), Halt, Discrete));
                }
                // c.) Continuation of a.) and loop of d.).
                (Selene::RUN_LEFT, ..) => {
                    entity_commands.insert((AnimationTag::new(Selene::RUN_RIGHT), Loop, Continuous));
                }
                // d.) Continuation of c.).
                (Selene::RUN_RIGHT, ..) => {
                    entity_commands.insert((AnimationTag::new(Selene::RUN_LEFT), Loop, Continuous));
                }
                _ => {}
            }
        }

        if state.is_changed() || dir.is_changed() {
            let new_scale_x = trns.scale.x.abs().copysign(dir.as_scalar());
            trns.map_unchanged(|t| &mut t.scale.x).set_if_neq(new_scale_x);

            match (tag.as_str(), *state_prev, *state) {
                // 1.) Any -> Start running.
                (.., Run { decelerating: false }) => {
                    entity_commands.insert((AnimationTag::new(Selene::RUN_TRANS_LEFT), Halt, Discrete));
                }
                // 2.) Walking -> Attempting to stop running (decelerating or instantly still).
                (tag, Run { decelerating: false }, Run { decelerating: true } | Idle) => {
                    if let Some(tag) = match tag {
                        Selene::RUN_LEFT => Some(Selene::RUN_TRANS_LEFT),
                        Selene::RUN_RIGHT => Some(Selene::RUN_TRANS_RIGHT),
                        Selene::RUN_TRANS_LEFT => None,
                        Selene::RUN_TRANS_RIGHT => None,
                        other => {
                            warn!("Invalid Selene animation state while running: {other}");
                            Some(Selene::RUN_TRANS_LEFT)
                        }
                    } {
                        entity_commands.insert((AnimationTag::new(tag), Halt, Discrete));
                    }
                }
                // 3.) Any -> Idling.
                (.., Idle) => {
                    entity_commands.insert((AnimationTag::new(Selene::IDLE), Halt, Discrete));
                }
                // 4.) Any -> Jumping.
                (.., Jump) => {
                    // TODO jump animation
                    entity_commands.insert((AnimationTag::new(Selene::IDLE), Halt, Discrete));
                }
                _ => {}
            }
        }
    }
}

fn adjust_selene_hair(
    sheets: Res<Assets<AnimationSheet>>,
    selenes: Query<(&Selene, AnimationQueryReadOnly)>,
    mut trns_query: Query<&mut Transform2d>,
) {
    for (selene, query) in selenes {
        if query.is_ticked()
            && let Ok(trns) = trns_query.get_mut(selene.hair)
            && let Some(assets) = query.assets(&sheets)
            && let Some(&slice) = assets.frame.slices.get("head_pos")
        {
            trns.map_unchanged(|t| &mut t.translation)
                .set_if_neq((slice.center() - vec2(0., 0.5)).extend(-1e-4));
        }
    }
}

fn draw_selene_hair(
    param: PainterParam,
    misc: Res<MiscTextures>,
    hairs: Query<(&SeleneHair, &Hair, &Painter, &GlobalTransform2d)>,
    strands: Query<&GlobalTransform2d>,
) {
    for (hair, hair_segments, painter, &trns) in hairs {
        let mut ctx = param.ctx(painter);
        ctx.color = hair.color;
        ctx.layer = trns.z;

        ctx.rect(&misc.circle, trns.affine, (Some(Vec2::splat(hair.widths[0])), default()));
        for (&next, &rad) in strands.iter_many(hair_segments.iter_strands()).zip(&hair.widths[1..]) {
            ctx.rect(&misc.circle, next.affine, (Some(Vec2::splat(rad)), default()));
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, spawn_selene.in_set(LevelSystems::SpawnEntities)).add_systems(
        PostUpdate,
        (
            (react_selene_animations, adjust_selene_hair).chain().in_set(AnimationSystems::PostUpdate),
            draw_selene_hair.after(TransformSystems::Propagate),
        ),
    );
}
