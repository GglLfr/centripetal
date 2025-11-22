mod asset;
mod serde;
pub use asset::*;
pub use serde::*;

use crate::prelude::*;

pub fn plugin(app: &mut App) {}
