mod selene;
pub use selene::*;

use crate::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins(selene::plugin);
}
