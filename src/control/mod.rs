mod ground;
pub use ground::*;

use crate::prelude::*;

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct Movement;

#[derive(InputAction)]
#[action_output(bool)]
pub struct Jump;

pub fn plugin(app: &mut App) {
    app.add_plugins(ground::plugin);
}
