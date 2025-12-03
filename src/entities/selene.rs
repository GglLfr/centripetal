use crate::{
    CharacterTextures,
    math::Transform2d,
    prelude::*,
    render::{
        CameraTarget, MAIN_LAYER,
        animation::{Animation, AnimationTag},
    },
    world::{EntityCreate, LevelSystems, MessageReaderEntityExt},
};

#[derive(Component, Debug)]
#[require(CameraTarget, RenderLayers = MAIN_LAYER)]
pub struct Selene;

impl Selene {
    pub const IDLE: &'static str = "idle";
}

fn on_selene_spawn(mut commands: Commands, mut messages: MessageReader<EntityCreate>, textures: Res<CharacterTextures>) {
    for &EntityCreate { entity, bounds, .. } in messages.created("selene") {
        commands.entity(entity).insert((
            Selene,
            Animation::from(&textures.selene),
            AnimationTag::new(Selene::IDLE),
            Transform2d::from_translation(bounds.center().extend(1.)),
        ));
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, on_selene_spawn.in_set(LevelSystems::SpawnEntities));
}
