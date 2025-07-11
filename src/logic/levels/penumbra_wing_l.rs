use std::time::Duration;

use bevy::{
    asset::uuid::{Uuid, uuid},
    ecs::{
        entity_disabling::Disabled,
        query::QueryItem,
        system::{
            SystemParamItem,
            lifetimeless::{Read, SRes},
        },
    },
    prelude::*,
};
use serde::{Deserialize, Serialize};

use crate::{
    SaveApp,
    logic::{CameraTarget, Fields, FromLevel, LevelApp, LevelEntities, LevelUnload},
};

pub const SELENE: Uuid = uuid!("332e5310-3740-11f0-b0d1-4b444b848a1e");
pub const CENTRAL_ATTRACTOR: Uuid = uuid!("8226eab0-3740-11f0-b0d1-31c3cf318fb2");

#[derive(Debug, Copy, Clone, Default, Resource, TypePath, Serialize, Deserialize, Deref, DerefMut)]
pub struct CutsceneShown(pub bool);

#[derive(Debug, Copy, Clone, Component)]
pub enum State {
    Init,
    Begin { started: Duration },
    AttractorSpawned { at: Duration },
    TutorialHover,
    TutorialAccelerate,
}

impl FromLevel for State {
    type Param = SRes<CutsceneShown>;
    type Data = Read<LevelEntities>;

    fn from_level(
        mut e: EntityCommands,
        _: &Fields,
        cutscene_shown: SystemParamItem<Self::Param>,
        entities: QueryItem<Self::Data>,
    ) -> Result {
        if !**cutscene_shown {
            let mut commands = e.commands();
            let selene = entities.get(SELENE)?;
            let attractor = entities.get(CENTRAL_ATTRACTOR)?;

            commands.get_entity(selene)?.insert_recursive::<Children>(Disabled);
            commands.get_entity(attractor)?.insert_recursive::<Children>(Disabled);

            e.insert(Self::Init);
            debug!("Loaded left-wing side Penumbra level (+ intro cutscene)!");
        } else {
            debug!("Loaded left-wing side Penumbra level!");
        }

        Ok(())
    }
}

pub const SPAWN_ATTRACTOR_DURATION: Duration = Duration::from_secs(2);
pub const SPAWN_SELENE_DURATION: Duration = Duration::from_secs(2);

pub fn update(
    mut commands: Commands,
    time: Res<Time>,
    level: Single<(&mut State, &LevelEntities), Without<LevelUnload>>,
) -> Result {
    let now = time.elapsed();
    let (mut level, entities) = level.into_inner();

    match *level {
        State::Init => *level = State::Begin { started: now },
        State::Begin { started } => {
            if now - started >= SPAWN_ATTRACTOR_DURATION {
                commands
                    .get_entity(entities.get(CENTRAL_ATTRACTOR)?)?
                    .remove_recursive::<Children, Disabled>()
                    .insert(CameraTarget);

                *level = State::AttractorSpawned { at: now };
            }
        }
        State::AttractorSpawned { at } => {
            if now - at >= SPAWN_SELENE_DURATION {
                commands
                    .get_entity(entities.get(CENTRAL_ATTRACTOR)?)?
                    .remove::<CameraTarget>();

                commands
                    .get_entity(entities.get(SELENE)?)?
                    .remove_recursive::<Children, Disabled>();

                *level = State::TutorialHover;
            }
        }
        State::TutorialHover => {}
        State::TutorialAccelerate => {}
    }

    Ok(())
}

pub(super) fn plugin(app: &mut App) {
    app.register_level::<State>("penumbra_wing_l")
        .add_systems(Update, update)
        .save_resource_init::<CutsceneShown>();
}
