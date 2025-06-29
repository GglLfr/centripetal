use avian2d::prelude::*;
use bevy::prelude::*;

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(RigidBody::Kinematic)]
pub struct PenumbraEntity;
