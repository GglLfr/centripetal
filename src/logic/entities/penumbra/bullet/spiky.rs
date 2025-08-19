use std::f32::consts::TAU;

use crate::{
    Affected, Observed, PIXELS_PER_UNIT, Sprites,
    graphics::{Animation, AnimationFrom, AnimationMode, AnimationSmoothing, BaseColor, SpriteDrawer, SpriteSection},
    logic::{
        Timed,
        entities::{
            EntityLayers, Health, TryHurt,
            penumbra::{HomingPower, NoAttract, PenumbraEntity},
        },
    },
    math::{FloatTransformExt as _, Interp, RngExt as _},
    prelude::*,
};

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(
    SpriteDrawer,
    DiscComponent {
        alignment: Alignment::Flat,
        cap: Cap::None,
        arc: false,
        radius: 0.,
        start_angle: 0.,
        end_angle: TAU,
    },
    ShapeMaterial {
        alpha_mode: ShapeAlphaMode::Blend,
        disable_laa: false,
        pipeline: ShapePipelineType::Shape2d,
        canvas: None,
        texture: None,
    },
    ShapeFill {
        color: Color::NONE,
        ty: FillType::Stroke(0., ThicknessType::World),
    },
    Timed::new(Duration::from_millis(500)),
)]
pub struct SpikyChargeEffect;

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
            if let Some(body) = trigger.body {
                commands.entity(body).queue_handled(TryHurt::by(trigger.target(), 1), ignore);
            }

            commands
                .entity(trigger.target())
                .queue_handled(TryHurt::by(trigger.target(), i32::MAX as u32), ignore);
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
                        AnimationSmoothing(Interp::Identity),
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

pub fn update_spiky_charge_effect(
    sprites: Res<Sprites>,
    sprite_sections: Res<Assets<SpriteSection>>,
    mut effects: Query<(Entity, &SpriteDrawer, &mut DiscComponent, &mut ShapeFill, &Timed), With<SpikyChargeEffect>>,
) {
    let Some(sprite) = sprite_sections.get(&sprites.ring_4) else { return };
    for (e, drawer, mut disc, mut fill, timed) in &mut effects {
        let f = timed.frac();
        let color = Color::linear_rgba(0., 1., 4., 0.).mix(&Color::linear_rgb(4., 2., 1.), f.pow_in(2));

        for (angle, offset) in Rng::with_seed(e.to_bits()).len_vectors(16, 0., TAU, 0., PIXELS_PER_UNIT as f32 * 5.) {
            drawer.draw_at(
                (offset * (1. - f).pow_out(3)).extend(0.),
                angle,
                sprite.sprite_with(color, None, default()),
            );
        }

        disc.radius = PIXELS_PER_UNIT as f32 * 3. * (1. - f).pow_out(3);
        fill.color = color;
        fill.ty = FillType::Stroke(f * 2., ThicknessType::World);
    }
}
