use crate::{
    CharacterTextures, MiscTextures,
    control::{GroundControl, GroundJump, GroundMove, Jump, Movement},
    entities::Hair,
    math::{GlobalTransform2d, Transform2d},
    prelude::*,
    render::{
        CameraTarget, MAIN_LAYER,
        animation::{Animation, AnimationRepeat, AnimationTag},
        painter::{Painter, PainterParam},
    },
    world::{EntityCreate, LevelSystems, MessageReaderEntityExt},
};

#[derive(Component, Debug, Clone, Copy)]
pub struct Selene;
impl Selene {
    pub const IDENT: &'static str = "selene";
    pub const IDLE: &'static str = "idle";
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

        let hair_back = vec![6., 5.25, 4.5, 3.75, 3., 2.5, 2., 1.5, 1.];
        commands.entity(entity).insert((
            Selene,
            // Hair.
            children![(
                Transform2d::from_xyz(0., 5., -f32::EPSILON),
                Hair::new(hair_back[1..].iter().map(|rad| rad * 0.5), 0.1),
                SeleneHair {
                    color: Srgba::hex("70A3C4").unwrap().into(),
                    widths: hair_back,
                },
            )],
            // Transforms.
            (
                Transform2d::from_translation(sprite_center.extend(1.)),
                TransformInterpolation,
                TransformHermiteEasing,
                CameraTarget::default(),
            ),
            // Rendering.
            (Animation::from(&textures.selene), AnimationTag::new(Selene::IDLE), AnimationRepeat::Loop),
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
                GroundControl,
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
    app.add_systems(Update, on_selene_spawn.in_set(LevelSystems::SpawnEntities))
        .add_systems(PostUpdate, draw_selene_hair.after(TransformSystems::Propagate));
}
