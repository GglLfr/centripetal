use crate::{
    CharacterTextures,
    control::{GroundControl, Jump, Movement},
    math::Transform2d,
    prelude::*,
    render::{
        CameraTarget, MAIN_LAYER,
        animation::{Animation, AnimationRepeat, AnimationTag},
        painter::PaintOffset,
    },
    world::{EntityCreate, LevelSystems, MessageReaderEntityExt},
};

#[derive(Component, Debug)]
#[component(storage = "SparseSet")]
pub struct Selene;

impl Selene {
    pub const IDENT: &'static str = "selene";
    pub const IDLE: &'static str = "idle";
}

fn on_selene_spawn(mut commands: Commands, mut messages: MessageReader<EntityCreate>, textures: Res<CharacterTextures>) {
    for &EntityCreate { entity, bounds, .. } in messages.created(Selene::IDENT) {
        let sprite_center = bounds.center();
        let collider_center = vec2(sprite_center.x, bounds.min.y + 12.);

        commands.entity(entity).insert((
            Selene,
            // Transforms.
            Transform2d::from_translation(collider_center.extend(1.)),
            TransformExtrapolation,
            CameraTarget::default(),
            // Rendering.
            Animation::from(&textures.selene),
            AnimationTag::new(Selene::IDLE),
            AnimationRepeat::Loop,
            PaintOffset(Transform2d::from_translation((sprite_center - collider_center).extend(0.))),
            MAIN_LAYER,
            // Collisions.
            Mass(50.),
            SweptCcd::LINEAR,
            Collider::round_rectangle(2., 20., 2.),
            #[cfg(feature = "dev")]
            DebugRender::default(),
            // Inputs.
            GroundControl::default(),
            actions!(GroundControl[(
                Action::<Movement>::new(),
                Down::new(0.5),
                Bindings::spawn(Cardinal::arrows()),
            ), (
                Action::<Jump>::new(),
                bindings![KeyCode::KeyZ],
            )]),
        ));
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, on_selene_spawn.in_set(LevelSystems::SpawnEntities));
}
