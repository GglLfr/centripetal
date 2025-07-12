use std::time::Duration;

use avian2d::prelude::*;
use bevy::{
    asset::uuid::{Uuid, uuid},
    ecs::{
        query::QueryItem,
        system::{
            SystemParamItem,
            lifetimeless::{Read, SRes},
        },
    },
    prelude::*,
};
use leafwing_input_manager::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    SaveApp, SpriteSheets,
    graphics::{Animation, EntityColor, OnAnimateDone},
    logic::{
        CameraTarget, Fields, FromLevel, InGameState, LevelApp, LevelEntities, LevelUnload,
        entities::penumbra::AttractedAction,
        levels::{disable, enable},
    },
};

#[derive(Debug, Copy, Clone, Default, Resource, TypePath, Serialize, Deserialize, Deref, DerefMut)]
pub struct IntroShown(pub bool);

#[derive(Debug, Copy, Clone, Component)]
pub enum State {
    Init,
    Begin { started: Duration },
    AttractorSpawned { at: Duration },
    TutorialHover,
    TutorialAccelerate,
}

impl FromLevel for State {
    type Param = SRes<IntroShown>;
    type Data = Read<LevelEntities>;

    fn from_level(
        mut e: EntityCommands,
        _: &Fields,
        cutscene_shown: SystemParamItem<Self::Param>,
        entities: QueryItem<Self::Data>,
    ) -> Result {
        if !**cutscene_shown {
            let mut commands = e.commands();
            for iid in [SELENE, ATTRACTOR, RINGS[0], RINGS[1], HOVER_TARGET] {
                commands.get_entity(entities.get(iid)?)?.queue(disable);
            }

            e.insert(Self::Init);
            debug!("Loaded left-wing side Penumbra level (+ intro cutscene)!");
        } else {
            todo!("Revisit this level for the non-intro variant...")
        }

        Ok(())
    }
}

pub const SELENE: Uuid = uuid!("332e5310-3740-11f0-b0d1-4b444b848a1e");
pub const ATTRACTOR: Uuid = uuid!("8226eab0-3740-11f0-b0d1-31c3cf318fb2");
pub const RINGS: [Uuid; 2] = [
    uuid!("483defc0-3740-11f0-bea9-1bca02df9366"),
    uuid!("516847d0-3740-11f0-bea9-db42cbfffb80"),
];
pub const HOVER_TARGET: Uuid = uuid!("ddc89020-3740-11f0-bea9-17dccf039850");

pub const SPAWN_ATTRACTOR_DURATION: Duration = Duration::from_secs(2);
pub const SPAWN_SELENE_DURATION: Duration = Duration::from_secs(2);

pub fn update(
    mut commands: Commands,
    time: Res<Time>,
    sprite_sheets: Res<SpriteSheets>,
    level: Single<(&mut State, &LevelEntities), Without<LevelUnload>>,
) -> Result {
    let now = time.elapsed();
    let (mut level, entities) = level.into_inner();

    let selene = entities.get(SELENE)?;
    let attractor = entities.get(ATTRACTOR)?;
    let hover_target = entities.get(HOVER_TARGET)?;

    match *level {
        State::Init => *level = State::Begin { started: now },
        State::Begin { started } => {
            if now - started >= SPAWN_ATTRACTOR_DURATION {
                commands
                    .get_entity(attractor)?
                    .queue(enable)
                    .insert(CameraTarget)
                    .with_children(|children| {
                        children
                            .spawn((
                                Animation::new(sprite_sheets.grand_attractor_spawned.clone(), "anim"),
                                EntityColor(Color::linear_rgba(1., 2., 24., 1.)),
                            ))
                            .observe(|trigger: Trigger<OnAnimateDone>, mut commands: Commands| {
                                commands.entity(trigger.target()).despawn();
                            });
                    });

                *level = State::AttractorSpawned { at: now };
            }
        }
        State::AttractorSpawned { at } => {
            if now - at >= SPAWN_SELENE_DURATION {
                commands.get_entity(attractor)?.remove::<CameraTarget>();

                commands
                    .get_entity(hover_target)?
                    .queue(enable)
                    .insert(CollisionEventsEnabled)
                    .observe(
                        move |trigger: Trigger<OnCollisionStart>,
                              mut commands: Commands,
                              level: Single<&mut State, Without<LevelUnload>>,
                              mut state: Query<&mut ActionState<AttractedAction>>|
                              -> Result {
                            if trigger.body.is_some_and(|body| body == selene) {
                                // Stop observing and despawn the generic entity.
                                commands.entity(trigger.observer()).despawn();
                                commands.get_entity(trigger.target())?.despawn();

                                // Enable accelerating.
                                *level.into_inner() = State::TutorialAccelerate;
                                state.get_mut(selene)?.enable_action(&AttractedAction::Prograde);
                            }

                            Ok(())
                        },
                    );

                commands
                    .get_entity(selene)?
                    .queue(enable)
                    // Can't use `Queue`s here because it's still `Disabled`.
                    .queue(|mut e: EntityWorldMut| -> Result {
                        let mut action = e
                            .get_mut::<ActionState<AttractedAction>>()
                            .ok_or("`ActionState<AttractedAction>` not found")?;

                        // Only `Hover` is enabled initially.
                        action.disable_action(&AttractedAction::Prograde);
                        action.disable_action(&AttractedAction::Launch);
                        action.disable_action(&AttractedAction::Parry);
                        Ok(())
                    });

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
        .add_systems(Update, update.run_if(in_state(InGameState::Resumed)))
        .save_resource_init::<IntroShown>();
}
