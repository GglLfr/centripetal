use crate::{
    Observed,
    graphics::{AnimationFrom, AnimationMode, BaseColor},
    logic::{
        Timed,
        entities::{
            EntityLayers, Health, TryHurt,
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
        (
            Timed::new(Duration::from_secs(2)),
            Observed::by(Timed::despawn_on_finished),
        ),
        HomingPower(180f32.to_radians()),
        AnimationFrom::sprite(|sprites| (sprites.bullet_spiky.clone_weak(), "anim")),
        AnimationMode::Repeat,
        TransformExtrapolation,
        BaseColor(Color::linear_rgba(4., 2., 1., 1.)),
        Health::new(1),
        Observed::by(
            |trigger: Trigger<OnCollisionStart>, mut commands: Commands| {
                commands
                    .entity(trigger.target())
                    .queue(TryHurt::new(i32::MAX as u32));
                if let Some(body) = trigger.body {
                    commands
                        .entity(body)
                        .queue(TryHurt::by(trigger.target(), 1));
                }
            },
        ),
        DebugRender::none(),
    )
}
