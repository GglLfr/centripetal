mod asset;
mod player;
pub use asset::*;
pub use player::*;

use crate::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins((asset::plugin, player::plugin));
}
