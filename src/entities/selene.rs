use crate::{
    CharacterTextures, GroundedController, Movement,
    math::Transform2d,
    prelude::*,
    render::{
        CameraTarget, MAIN_LAYER,
        animation::{Animation, AnimationTag},
    },
    world::{EntityCreate, LevelSystems, MessageReaderEntityExt},
};

#[derive(Component, Debug)]
#[component(storage = "SparseSet")]
#[require(CameraTarget, RenderLayers = MAIN_LAYER, GroundedController)]
pub struct Selene;

impl Selene {
    pub const IDENT: &'static str = "selene";
    pub const IDLE: &'static str = "idle";
}

fn on_selene_spawn(mut commands: Commands, mut messages: MessageReader<EntityCreate>, textures: Res<CharacterTextures>) {
    for &EntityCreate { entity, bounds, .. } in messages.created(Selene::IDENT) {
        commands.entity(entity).insert((
            Selene,
            Transform2d::from_translation(bounds.center().extend(1.)),
            Animation::from(&textures.selene),
            AnimationTag::new(Selene::IDLE),
            actions!(GroundedController[(Action::<Movement>::new(), Bindings::spawn(Cardinal::arrows()),)]),
            children![(
                Transform::from_xyz(0., 12. - bounds.half_size().y, 0.),
                Collider::capsule(3., 18.),
                #[cfg(feature = "dev")]
                DebugRender::default(),
            )],
            #[cfg(feature = "dev")]
            DebugRender::none(),
        ));
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, on_selene_spawn.in_set(LevelSystems::SpawnEntities));
}
