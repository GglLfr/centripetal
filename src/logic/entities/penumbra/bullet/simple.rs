use crate::{
    Observed,
    graphics::{AnimationFrom, AnimationMode, BaseColor},
    logic::{
        Timed,
        entities::{
            EntityLayers, Health,
            penumbra::{HomingPower, NoAttract, PenumbraEntity},
        },
    },
    prelude::*,
};

pub fn spiky(level_entity: Entity) -> impl Bundle {
    (
        ChildOf(level_entity),
        PenumbraEntity,
        NoAttract,
        EntityLayers::penumbra_hostile(),
        Collider::circle(6.),
        CollisionEventsEnabled,
        Timed::new(Duration::from_secs(2)),
        HomingPower(180f32.to_radians()),
        AnimationFrom::sprite(|sprites| (sprites.bullet_spiky.clone_weak(), "anim")),
        AnimationMode::Repeat,
        TransformExtrapolation,
        BaseColor(Color::linear_rgba(4., 2., 1., 1.)),
        Observed::by(Timed::despawn_on_finished),
        Health::new(1),
        DebugRender::none(),
    )
}
