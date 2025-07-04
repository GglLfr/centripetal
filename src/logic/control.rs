use bevy::{
    ecs::{component::HookContext, world::DeferredWorld},
    prelude::*,
};
use leafwing_input_manager::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{Config, PlayerKeybinds, logic::entities::penumbra::AttractedAction};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Reflect, Actionlike, Serialize, Deserialize)]
pub enum PlayerAction {
    #[actionlike(DualAxis)]
    Move,
    Attack,
}

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(InputMap<PlayerAction>, InputMap<AttractedAction>)]
#[component(on_insert = sync_input_map)]
pub struct IsPlayer;
pub fn sync_input_map(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    let keybinds = (*world.resource::<Config<PlayerKeybinds>>()).clone();
    let mut e = world.entity_mut(entity);
    *e.get_mut::<InputMap<PlayerAction>>().unwrap() = keybinds.player;
    *e.get_mut::<InputMap<AttractedAction>>().unwrap() = keybinds.attracted;

    debug!("Synchronized player input map!");
}
