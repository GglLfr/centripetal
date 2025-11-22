mod page;
mod region;
pub use page::*;
pub use region::*;

use crate::prelude::*;

pub fn plugin(app: &mut App) {
    app.add_plugins((page::plugin, region::plugin));
}
