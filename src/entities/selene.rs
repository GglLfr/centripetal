use crate::{
    prelude::*,
    world::{EntityCreate, LevelSystems},
};

#[derive(Component, Debug)]
pub struct Selene;

fn on_selene_spawn(mut commands: Commands, mut messages: MessageReader<EntityCreate>) {
    if let Some(EntityCreate { entity, identifier }) = messages.read().last()
        && identifier == "selene"
    {
        info!("Spawned Selene at {}", commands.entity(*entity).insert(Selene).id());
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, on_selene_spawn.in_set(LevelSystems::SpawnEntities));
}
