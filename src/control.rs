use crate::prelude::*;

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct Movement;

#[derive(Reflect, Resource, Asset)]
pub struct Keybinds {
    //
}
