mod apply;
mod asset;
mod capture;
mod serde;
pub use apply::*;
pub use asset::*;
pub use capture::*;
pub use serde::*;

use crate::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins((asset::plugin, capture::plugin));
}
