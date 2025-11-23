mod bundle;
mod component;
pub use bundle::*;
pub use component::*;

use crate::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins((bundle::plugin, component::plugin));
}
