#[cfg(feature = "dev")]
mod editor;
mod tilemap;
#[cfg(feature = "dev")]
pub use editor::*;
pub use tilemap::*;

use crate::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins((
        tilemap::plugin,
        #[cfg(feature = "dev")]
        editor::plugin,
    ));
}
