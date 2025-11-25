mod iter;
pub use iter::*;

pub mod async_bridge;
pub mod ecs;

use crate::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins((async_bridge::plugin, ecs::plugin));
}
