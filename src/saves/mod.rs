mod asset;
mod serde;
mod system;
pub use asset::*;
pub use serde::*;
pub use system::*;

use crate::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins((asset::plugin, system::plugin));
}
