use crate::{
    math::Transform2d,
    prelude::*,
    render::{CameraTarget, MAIN_LAYER, painter::Painter},
    world::{EntityCreate, LevelSystems, MessageReaderEntityExt},
};

#[derive(Component, Debug)]
#[require(CameraTarget, Painter, RenderLayers = MAIN_LAYER)]
pub struct Selene;

fn on_selene_spawn(mut commands: Commands, mut messages: MessageReader<EntityCreate>) {
    for &EntityCreate { entity, bounds, .. } in messages.created("selene") {
        commands
            .entity(entity)
            .insert((Selene, Transform2d::from_translation(bounds.center().extend(0.))));
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, on_selene_spawn.in_set(LevelSystems::SpawnEntities));
}
