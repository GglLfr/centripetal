mod hair;
pub use hair::*;

pub mod characters;

use crate::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins((characters::plugin, hair::plugin));
}
