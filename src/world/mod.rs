mod level;
mod level_collection;
mod tilemap;
pub use level::*;
pub use level_collection::*;
pub use tilemap::*;

use crate::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins((level::plugin, level_collection::plugin, tilemap::plugin));
}
