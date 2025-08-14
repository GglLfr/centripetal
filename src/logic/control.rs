use crate::{
    Config, KeyboardBindings,
    logic::entities::penumbra::{AttractedAction, LaunchAction},
    prelude::*,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Reflect, Actionlike, Serialize, Deserialize)]
pub enum PlayerAction {
    #[actionlike(DualAxis)]
    Move,
    Attack,
}

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(InputMap<PlayerAction>, InputMap<AttractedAction>, InputMap<LaunchAction>)]
#[component(on_insert = sync_input_map)]
pub struct IsPlayer;
pub fn sync_input_map(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    let keybinds = world
        .resource::<Config<KeyboardBindings>>()
        .create_input_maps();

    world.commands().entity(entity).insert(keybinds);
}
