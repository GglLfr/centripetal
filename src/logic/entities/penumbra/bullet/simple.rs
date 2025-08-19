use std::f32::consts::TAU;

use crate::{
    Affected, Observed, Sprites,
    graphics::{Animation, AnimationFrom, AnimationMode, BaseColor},
    logic::{
        Timed,
        entities::{
            EntityLayers, Health, TryHurt,
            penumbra::{HomingPower, NoAttract, PenumbraEntity},
        },
    },
    math::{FloatTransformExt as _, RngExt as _},
    prelude::*,
};

#[derive(Debug, Copy, Clone, Default, Component)]
pub struct SpikySpawnEffect;

pub fn spiky(level_entity: Entity) -> impl Bundle {
    (
        ChildOf(level_entity),
        PenumbraEntity,
        NoAttract,
        (
            EntityLayers::penumbra_hostile(),
            Collider::circle(6.),
            CollisionEventsEnabled,
            TransformExtrapolation,
        ),
        (Timed::new(Duration::from_secs(2)), Observed::by(Timed::kill_on_finished)),
        HomingPower(180f32.to_radians()),
        (
            AnimationFrom::sprite(|sprites| (sprites.bullet_spiky.clone_weak(), "anim")),
            AnimationMode::Repeat,
        ),
        BaseColor(Color::linear_rgba(4., 2., 1., 1.)),
        Health::new(1),
        Observed::by(|trigger: Trigger<OnCollisionStart>, mut commands: Commands| {
            commands
                .entity(trigger.target())
                .queue_handled(TryHurt::by(trigger.target(), i32::MAX as u32), ignore);
            if let Some(body) = trigger.body {
                commands.entity(body).queue_handled(TryHurt::by(trigger.target(), 1), ignore);
            }
        }),
        Affected::by(
            move |In(e): In<Entity>, mut commands: Commands, sprites: Res<Sprites>, transforms: Query<&Transform>| -> Result {
                let mut rng = Rng::with_seed(e.to_bits());
                let range = TAU / 12.;
                let angle = rng.f32_within(-range, range);

                let &trns = transforms.get(e)?;
                commands
                    .spawn((
                        ChildOf(level_entity),
                        SpikySpawnEffect,
                        Animation::new(sprites.bullet_spawn_cloudy.clone_weak(), "anim"),
                        Timed::from_anim("anim"),
                        BaseColor(Color::linear_rgb(24., 4., 2.)),
                        Transform {
                            rotation: Quat::from_axis_angle(Vec3::Z, angle),
                            ..trns
                        },
                    ))
                    .observe(Timed::despawn_on_finished);

                Ok(())
            },
        ),
        DebugRender::none(),
    )
}

pub fn color_spiky_spawn_effect(mut effects: Query<(&mut BaseColor, &Timed), With<SpikySpawnEffect>>) {
    for (mut color, timed) in &mut effects {
        **color = Color::linear_rgb(48., 8., 4.).mix(&Color::linear_rgba(4., 2., 1., 1.), timed.frac().pow_out(6));
    }
}
