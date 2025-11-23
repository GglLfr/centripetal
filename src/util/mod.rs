mod iter;
pub use iter::*;

pub mod ecs;

use crate::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins(ecs::plugin);
}
