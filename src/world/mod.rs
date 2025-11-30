mod asset;
mod tilemap;
mod tileset;
pub use asset::*;
pub use tilemap::*;
pub use tileset::*;

use crate::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins((asset::plugin, tilemap::plugin, tileset::plugin));
}
