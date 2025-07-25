use std::time::Duration;

use bevy::prelude::*;

use crate::{
    Observed,
    graphics::{AnimationFrom, AnimationMode, EntityColor},
    logic::{
        Timed,
        entities::penumbra::{HomingPower, PenumbraEntity},
    },
};

pub fn spiky(level_entity: Entity) -> impl Bundle {
    (
        ChildOf(level_entity),
        PenumbraEntity,
        Timed::new(Duration::from_secs(2)),
        HomingPower(180f32.to_radians()),
        AnimationFrom::sprite(|sprites| (sprites.bullet_spiky.clone_weak(), "anim")),
        AnimationMode::Repeat,
        EntityColor(Color::linear_rgba(4., 2., 1., 1.)),
        Observed::by(Timed::despawn_on_finished),
    )
}
