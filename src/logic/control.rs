use bevy::{
    ecs::{component::HookContext, world::DeferredWorld},
    prelude::*,
};
use leafwing_input_manager::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{Config, PlayerKeybinds};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Reflect, Actionlike, Serialize, Deserialize)]
pub enum PlayerAction {
    #[actionlike(DualAxis)]
    Move,
    Attack,
    PenumbraHoverIntensify,
    #[actionlike(Axis)]
    PenumbraHover,
}

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(ActionState<PlayerAction>, InputMap<PlayerAction>)]
#[component(on_insert = sync_input_map)]
pub struct IsPlayer;
pub fn sync_input_map(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    let keybinds = (***world.resource::<Config<PlayerKeybinds>>()).clone();
    *world.entity_mut(entity).get_mut::<InputMap<PlayerAction>>().unwrap() = keybinds;
}
