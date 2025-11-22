mod tilemap;
pub use tilemap::*;

use crate::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins(tilemap::plugin);
}
