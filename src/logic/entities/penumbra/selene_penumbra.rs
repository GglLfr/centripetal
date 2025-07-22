use std::time::Duration;

use avian2d::prelude::*;
use bevy::{
    ecs::{
        query::QueryItem,
        system::{SystemParamItem, lifetimeless::SRes},
    },
    prelude::*,
};

use crate::{
    Sprites,
    graphics::{Animation, AnimationMode, EntityColor},
    logic::{
        CameraTarget, Fields, FromLevelEntity, IsPlayer,
        entities::{
            Health, Hurt, MaxHealth,
            penumbra::{
                AttractedInitial, AttractedParams, AttractedPrediction, LaunchCooldown,
                LaunchDurations, LaunchTarget, Launched, PenumbraEntity,
            },
        },
    },
};

#[derive(Debug, Copy, Clone, Default, Component)]
pub struct LaunchDisc;

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(
    IsPlayer,
    CameraTarget,
    PenumbraEntity,
    LaunchTarget,
    AttractedParams {
        ascend: 240.,
        descend: 240.,
        prograde: 80.,
        retrograde: 80.,
        precise_scale: 1. / 5.,
    },
    AttractedPrediction {
        points: Vec::new(),
        max_distance: 480.,
    },
    LaunchDurations([250, 500, 750].into_iter().map(Duration::from_millis).collect()),
    LaunchCooldown(Duration::from_secs(1)),
    Health::new(10),
    MaxHealth::new(10),
    Collider::circle(5.),
)]
pub struct SelenePenumbra;
impl FromLevelEntity for SelenePenumbra {
    type Param = SRes<Sprites>;
    type Data = ();

    fn from_level_entity(
        mut e: EntityCommands,
        fields: &Fields,
        sprites: &mut SystemParamItem<Self::Param>,
        _: QueryItem<Self::Data>,
    ) -> Result {
        let ccw = fields.bool("ccw")?;
        e.insert((
            Self,
            AttractedInitial { ccw },
            Animation::new(sprites.selene_penumbra.clone_weak(), "anim"),
            AnimationMode::Repeat,
            EntityColor(Color::linear_rgba(1., 2., 24., 1.)),
            DebugRender::none(),
        ))
        .observe(on_selene_launch)
        .observe(on_selene_hurt);

        Ok(())
    }
}

pub fn draw_launch_disc() {}

pub fn on_selene_hurt(trigger: Trigger<Hurt>) {
    debug!(
        "Selene ({}) hurt by {}!",
        trigger.target(),
        trigger.event().by
    );
}

pub fn on_selene_launch(trigger: Trigger<Launched>) {
    debug!(
        "Selene ({}) launched without obstruction!",
        trigger.target()
    );
}
