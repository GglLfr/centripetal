mod async_bridge;
mod iter;
pub use async_bridge::*;
pub use iter::*;

pub mod ecs;

use crate::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins((async_bridge::plugin, ecs::plugin));
}
