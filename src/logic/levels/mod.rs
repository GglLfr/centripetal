use avian2d::prelude::*;
use bevy::{ecs::entity_disabling::Disabled, prelude::*};

use crate::logic::{Level, LevelUnload};

mod penumbra_wing_l;

#[derive(Debug, Copy, Clone, Default)]
pub struct LevelsPlugin;
impl Plugin for LevelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((penumbra_wing_l::plugin,));
    }
}

pub fn in_level(level_id: impl Into<String>) -> impl FnMut(Query<&Level, Without<LevelUnload>>) -> bool + Clone {
    let id = level_id.into();
    move |level: Query<&Level, Without<LevelUnload>>| {
        let Ok(level) = level.single() else { return false };
        level.id == id
    }
}

pub fn disable(mut e: EntityWorldMut) {
    e.insert_recursive::<Children>(Disabled);
}

pub fn enable(mut e: EntityWorldMut) {
    // HACK: Avian currently breaks with disabled entities. This method restarts the whole mechanism.
    if let Some(Disabled) = e.take::<Disabled>() &&
        let Some(body) = e.take::<(RigidBody, Collider, Transform, GlobalTransform)>()
    {
        e.remove_recursive::<Children, Disabled>();
        e.remove_with_requires::<(RigidBody, Collider, Transform)>();
        e.insert(body);
    }
}
