mod transform;
pub use transform::*;

use crate::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins(transform::plugin);
}
