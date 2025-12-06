mod iter;
pub use iter::*;

pub mod async_bridge;
pub mod ecs;

use crate::prelude::*;

/// [`std::mem::replace`] but with inversed parameter order to not piss off Rust's borrow rules.
#[inline(always)]
pub const fn replace_with<T>(src: T, dst: &mut T) -> T {
    mem::replace(dst, src)
}

pub fn plugin(app: &mut App) {
    app.add_plugins((async_bridge::plugin, ecs::plugin));
}
