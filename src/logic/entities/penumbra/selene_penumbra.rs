use avian2d::prelude::*;
use bevy::{
    ecs::{query::QueryItem, system::SystemParamItem},
    prelude::*,
};

use crate::logic::{CameraTarget, FromLevelEntity, LevelEntity};

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(CameraTarget)]
pub struct SelenePenumbra;
impl FromLevelEntity for SelenePenumbra {
    type Param = ();
    type Data = ();

    fn from_level_entity(
        mut e: EntityCommands,
        _: &LevelEntity,
        _: &mut SystemParamItem<Self::Param>,
        _: QueryItem<Self::Data>,
    ) -> Result {
        e.insert((Self, RigidBody::Kinematic, Collider::circle(8.)));
        Ok(())
    }
}
