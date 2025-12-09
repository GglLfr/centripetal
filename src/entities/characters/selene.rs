use crate::{
    CharacterTextures, MiscTextures,
    control::{GroundControl, GroundControlDirection, GroundControlState, GroundJump, GroundMove, Jump, Movement},
    entities::Hair,
    math::{GlobalTransform2d, Transform2d},
    prelude::*,
    render::{
        CameraTarget, MAIN_LAYER,
        animation::{Animation, AnimationQueryReadOnly, AnimationRepeat, AnimationSheet, AnimationSystems, AnimationTag, AnimationTransition},
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
    pub const RUN: &'static str = "run";
}

#[derive(Component, Debug, Clone)]
#[require(Painter, Transform2d)]
pub struct SeleneHair {
    pub color: LinearRgba,
    pub widths: Vec<f32>,
}

fn on_selene_spawn(mut commands: Commands, mut messages: MessageReader<EntityCreate>, textures: Res<CharacterTextures>) {
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

        commands.entity(hair).insert((
            ChildOf(entity),
            Transform2d::IDENTITY,
            Hair::new(hair_back[1..].iter().map(|rad| rad / 3.), 0.1),
            SeleneHair {
                color: Srgba::hex("70A3C4").unwrap().into(),
                widths: hair_back,
            },
        ));
    }
}

fn react_selene_animations(
    mut commands: Commands,
    states: Query<(Entity, &mut Transform, Ref<GroundControlState>, Ref<GroundControlDirection>), With<Selene>>,
) {
    for (selene, mut trns, state, dir) in states {
        if !state.is_changed() && !dir.is_changed() {
            continue
        }

        trns.scale = Vec3 {
            x: trns.scale.x.abs().copysign(dir.as_scalar()),
            ..trns.scale
        };

        commands.entity(selene).insert(match *state {
            GroundControlState::Idle => (AnimationTag::new(Selene::IDLE), AnimationRepeat::Halt, AnimationTransition::Discrete),
            GroundControlState::Walk { .. } => (AnimationTag::new(Selene::RUN), AnimationRepeat::Loop, AnimationTransition::Discrete),
            _ => (AnimationTag::new(Selene::IDLE), AnimationRepeat::Halt, AnimationTransition::Discrete),
            //GroundControlState::Hover { steering } => todo!(),
            //GroundControlState::Jump => todo!(),
        });
    }
}

fn adjust_selene_scale(
    sheets: Res<Assets<AnimationSheet>>,
    selenes: Query<(&Selene, AnimationQueryReadOnly)>,
    mut trns_query: Query<&mut Transform2d>,
) {
    for (selene, query) in selenes {
        if query.is_ticked()
            && let Ok(mut trns) = trns_query.get_mut(selene.hair)
            && let Some(assets) = query.assets(&sheets)
            && let Some(&slice) = assets.frame.slices.get("head_pos")
        {
            trns.set_if_neq(Transform2d::from_translation((slice.center() - vec2(0., 0.5)).extend(-f32::EPSILON)));
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
    app.add_systems(Update, on_selene_spawn.in_set(LevelSystems::SpawnEntities)).add_systems(
        PostUpdate,
        (
            react_selene_animations.before(AnimationSystems::Update),
            adjust_selene_scale.in_set(AnimationSystems::Updated),
            draw_selene_hair.after(TransformSystems::Propagate),
        ),
    );
}
