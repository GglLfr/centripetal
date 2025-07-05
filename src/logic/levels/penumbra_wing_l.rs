use bevy::{
    ecs::{query::QueryItem, system::SystemParamItem},
    prelude::*,
};

use crate::logic::{Fields, FromLevel};

#[derive(Debug, Copy, Clone, Default, Component)]
pub struct PenumbraWingL;
impl FromLevel for PenumbraWingL {
    type Param = ();
    type Data = ();

    fn from_level(mut e: EntityCommands, _: &Fields, _: SystemParamItem<Self::Param>, _: QueryItem<Self::Data>) -> Result {
        e.insert(Self);

        debug!("Loaded left-wing side Penumbra level!");
        Ok(())
    }
}
