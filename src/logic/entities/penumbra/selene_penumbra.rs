use std::time::Duration;

use avian2d::prelude::*;
use bevy::{
    ecs::{query::QueryItem, system::SystemParamItem},
    prelude::*,
};

use crate::logic::{
    CameraTarget, Fields, FromLevelEntity, IsPlayer,
    entities::{
        Hurt,
        penumbra::{AttractedInitial, AttractedLaunch, AttractedParams, AttractedPrediction, OnLaunch, PenumbraEntity},
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
        fields: &Fields,
        _: &mut SystemParamItem<Self::Param>,
        _: QueryItem<Self::Data>,
    ) -> Result {
        let ccw = fields.bool("ccw")?;
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
        .observe(on_selene_launch)
        .observe(on_selene_hurt);

        debug!("Spawned Selene {}!", e.id());
        Ok(())
    }
}

pub fn on_selene_hurt(trigger: Trigger<Hurt>) {
    debug!("Selene ({}) hurt by {}!", trigger.target(), trigger.event().by);
}

pub fn on_selene_launch(trigger: Trigger<OnLaunch>) {
    debug!("Selene ({}) launched without obstruction!", trigger.target());
}
