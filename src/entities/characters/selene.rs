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
pub struct Selene {
    pub hairs: [Entity; 5],
}

impl Selene {
    pub const IDENT: &'static str = "selene";
    pub const IDLE: &'static str = "idle";
}

fn on_selene_spawn(mut commands: Commands, mut messages: MessageReader<EntityCreate>, textures: Res<CharacterTextures>) {
    for &EntityCreate { entity, bounds, .. } in messages.created(Selene::IDENT) {
        let sprite_center = bounds.center();
        let collider_center = vec2(sprite_center.x, bounds.min.y + 12.);

        let hairs = array::from_fn(|_| commands.spawn_empty().id());
        commands.entity(entity).insert((
            Selene { hairs },
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

        for (i, e) in hairs.into_iter().enumerate() {
            commands.entity(e).insert((ChildOf(entity), match i {
                0 => (Transform2d::from_xy(-3., 4.), Hair::new(6, 1., 0.1)),
                1 => (Transform2d::from_xy(-1., 4.), Hair::new(6, 1., 0.1)),
                2 => (Transform2d::from_xy(0.5, 4.), Hair::new(6, 1., 0.1)),
                3 => (Transform2d::from_xy(2., 4.), Hair::new(6, 1., 0.1)),
                4 => (Transform2d::from_xy(4., 4.), Hair::new(6, 1., 0.1)),
                _ => continue,
            }));
        }
    }
}

fn draw_selene_hair(
    param: PainterParam,
    misc: Res<MiscTextures>,
    selenes: Query<(&Selene, &Painter, &GlobalTransform2d)>,
    hair_query: Query<&Hair>,
    trns_query: Query<&GlobalTransform2d>,
) {
    let hair_back = LinearRgba::from(Srgba::from_u8_array(u32::to_be_bytes(0x70a3c4ff)));
    for (selene, painter, &selene_trns) in selenes {
        let mut ctx = param.ctx(painter);
        ctx.color = LinearRgba::new(1., 1., 1., 0.5); //hair_back;
        ctx.layer = selene_trns.z.next_up();

        for hairs in selene.hairs.windows(2) {
            let &[a, b] = hairs else { continue };

            let [Ok(mut prev_a), Ok(mut prev_b)] = [a, b].map(|e| trns_query.get(e).map(|t| t.affine.translation)) else { continue };
            let [Ok(mut a), Ok(mut b)] = [a, b].map(|e| {
                hair_query.get(e).map(|hair| {
                    hair.iter_strands()
                        .flat_map(|strand| trns_query.get(strand).map(|t| t.affine.translation))
                })
            }) else {
                continue
            };

            let [Some(mut curr_a), Some(mut curr_b)] = [a.next(), b.next()] else { continue };
            loop {
                ctx.quad(&misc.white, [prev_a, curr_a, curr_b, prev_b]);
                match (a.next(), b.next()) {
                    (None, None) => break,
                    (Some(next_a), Some(next_b)) => {
                        prev_a = mem::replace(&mut curr_a, next_a);
                        prev_b = mem::replace(&mut curr_b, next_b);
                    }
                    (Some(next_a), None) => prev_a = mem::replace(&mut curr_a, next_a),
                    (None, Some(next_b)) => prev_b = mem::replace(&mut curr_b, next_b),
                }
            }
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, on_selene_spawn.in_set(LevelSystems::SpawnEntities))
        .add_systems(PostUpdate, draw_selene_hair.after(TransformSystems::Propagate));
}
