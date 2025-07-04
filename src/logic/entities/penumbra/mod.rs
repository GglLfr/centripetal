use avian2d::prelude::*;
use bevy::prelude::*;

mod attractor;
mod selene_penumbra;
mod thorn_pillar;
mod thorn_ring;

pub use attractor::*;
pub use selene_penumbra::*;
pub use thorn_pillar::*;
pub use thorn_ring::*;

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(RigidBody::Kinematic)]
pub struct PenumbraEntity;
