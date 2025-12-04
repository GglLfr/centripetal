mod context;
mod pipeline;
mod vertex;
pub use context::*;
pub use pipeline::*;
pub use vertex::*;

use crate::{math::Transform2d, prelude::*};

#[derive(Reflect, Component, Debug, Default, Clone, Copy, Deref, DerefMut)]
#[reflect(Component, Debug, Default, FromWorld, Clone)]
pub struct PaintOffset(pub Transform2d);

pub fn plugin(app: &mut App) {
    app.add_plugins((pipeline::plugin, vertex::plugin));
}
