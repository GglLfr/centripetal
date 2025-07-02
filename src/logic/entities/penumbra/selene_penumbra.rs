use std::time::Duration;

use avian2d::prelude::*;
use bevy::{
    ecs::{query::QueryItem, system::SystemParamItem},
    prelude::*,
};
use leafwing_input_manager::{action_state::ActionData, prelude::*};

use crate::logic::{
    CameraTarget, FromLevelEntity, IsPlayer, LevelEntity, PenumbraEntity, PlayerAction,
    entities::penumbra::{
        AttractedAction, AttractedInitial, AttractedLaunch, AttractedParams, AttractedPrediction, OnLaunch,
    },
};

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(IsPlayer, CameraTarget, PenumbraEntity)]
pub struct SelenePenumbra;
impl FromLevelEntity for SelenePenumbra {
    type Param = ();
    type Data = ();

    fn from_level_entity(
        mut e: EntityCommands,
        entity: &LevelEntity,
        _: &mut SystemParamItem<Self::Param>,
        _: QueryItem<Self::Data>,
    ) -> Result {
        let ccw = entity.bool("ccw")?;
        e.insert((
            Self,
            AttractedInitial { ccw },
            AttractedParams {
                centrifugal: 240.,
                centripetal: 240.,
                prograde: 80.,
                retrograde: 80.,
                precise_scale: 1. / 5.,
                launches: vec![
                    AttractedLaunch {
                        charge: Duration::from_millis(250),
                        damage: 1,
                    },
                    AttractedLaunch {
                        charge: Duration::from_millis(500),
                        damage: 4,
                    },
                    AttractedLaunch {
                        charge: Duration::from_millis(750),
                        damage: 8,
                    },
                ],
                launch_cooldown: Duration::from_secs(1),
            },
            AttractedPrediction {
                points: Vec::new(),
                max_distance: 640.,
            },
            Collider::circle(5.),
        ))
        .observe(on_selene_launch);

        debug!("Spawned Selene {}!", e.id());
        Ok(())
    }
}

pub fn copy_player_to_hover_state(mut selene: Query<(&mut ActionState<AttractedAction>, &ActionState<PlayerAction>)>) {
    for (mut hover_state, player_state) in &mut selene {
        for (src, dst) in [
            (PlayerAction::PenumbraPrograde, AttractedAction::Prograde),
            (PlayerAction::PenumbraHover, AttractedAction::Hover),
            (PlayerAction::PenumbraPrecise, AttractedAction::Precise),
            (PlayerAction::PenumbraLaunch, AttractedAction::Launch),
        ] {
            let mut tmp = None;
            hover_state
                .action_data_mut_or_default(&dst)
                .clone_from(match player_state.action_data(&src) {
                    Some(state) => state,
                    None => tmp.insert(ActionData::from_kind(src.input_control_kind())),
                });
        }
    }
}

pub fn on_selene_launch(trigger: Trigger<OnLaunch>) {
    debug!("Selene ({}) launched without obstruction!", trigger.target());
}
