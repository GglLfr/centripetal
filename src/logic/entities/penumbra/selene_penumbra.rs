use avian2d::prelude::*;
use bevy::{
    ecs::{query::QueryItem, system::SystemParamItem},
    prelude::*,
};
use leafwing_input_manager::prelude::*;

use crate::logic::{
    CameraTarget, FromLevelEntity, IsPlayer, LevelEntity, PenumbraEntity, PlayerAction,
    entities::penumbra::{AttractorHoverAction, AttractorHoverParams, AttractorInitial},
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
            AttractorInitial { ccw },
            AttractorHoverParams {
                centrifugal: 80.,
                centripetal: 80.,
                centrifugal_intense: 24.,
                centripetal_intense: 24.,
            },
            Collider::circle(8.),
        ));
        Ok(())
    }
}

pub fn copy_player_to_hover_state(mut selene: Query<(&mut ActionState<AttractorHoverAction>, &ActionState<PlayerAction>)>) {
    for (mut hover_state, player_state) in &mut selene {
        if let Some(data) = player_state.button_data(&PlayerAction::PenumbraHoverIntensify) {
            hover_state
                .button_data_mut_or_default(&AttractorHoverAction::Intensify)
                .clone_from(data);
        }

        if let Some(data) = player_state.axis_data(&PlayerAction::PenumbraHover) {
            hover_state
                .axis_data_mut_or_default(&AttractorHoverAction::Maneouver)
                .clone_from(data);
        }
    }
}
