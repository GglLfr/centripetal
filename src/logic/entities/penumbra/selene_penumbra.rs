use std::f32::consts::TAU;

use crate::{
    Sprites,
    graphics::{Animation, AnimationHooks, AnimationMode, AnimationSmoothing, BaseColor, SpriteDrawer, SpriteSection},
    logic::{
        CameraTarget, Fields, FromLevelEntity, IsPlayer, Level, LevelUnload, TimeStun, Timed,
        entities::{
            EntityLayers, Health, Hurt, MaxHealth, TryHurt,
            penumbra::{
                AttractedInitial, AttractedParams, AttractedPrediction, LaunchCharging, LaunchCooldown, LaunchDurations, LaunchTarget, Launched,
                PenumbraEntity, TryLaunch,
            },
        },
    },
    math::{FloatTransformExt as _, Interp},
    prelude::*,
};

#[derive(Debug, Copy, Clone, Default, Component)]
pub struct LaunchDisc;

#[derive(Debug, Copy, Clone, Default, Component)]
pub struct HurtEffect;

#[derive(Debug, Copy, Clone, Default, Component)]
pub struct SlashEffect;

#[derive(Debug, Copy, Clone, Default, Component)]
pub struct SeleneParry {
    pub radius: f32,
    pub warned: bool,
    pub warn_radius: f32,
    pub warn_time: [Duration; 2],
}

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(
    IsPlayer,
    CameraTarget,
    PenumbraEntity,
    LaunchTarget,
    SpriteDrawer,
    SeleneParry {
        radius: 24.,
        warned: false,
        warn_radius: 16.,
        warn_time: default(),
    },
    AttractedParams {
        ascend: 240.,
        descend: 240.,
        prograde: 80.,
        retrograde: 80.,
        precise_scale: 1. / 5.,
    },
    AttractedPrediction {
        points: Vec::new(),
        max_distance: 240.,
    },
    LaunchDurations([250, 500, 750].into_iter().map(Duration::from_millis).collect()),
    LaunchCooldown(Duration::from_secs(1)),
    Health::new(10),
    MaxHealth::new(10),
    Collider::circle(5.),
    CollisionLayers = EntityLayers::penumbra_selene(),
    CollisionEventsEnabled,
    TransformExtrapolation,
)]
pub struct SelenePenumbra;

impl FromLevelEntity for SelenePenumbra {
    type Param = (SRes<Sprites>, ShapeCommands<'static, 'static>);
    type Data = ();

    fn from_level_entity(
        mut e: EntityCommands,
        fields: &Fields,
        (sprites, shapes): &mut SystemParamItem<Self::Param>,
        _: QueryItem<Self::Data>,
    ) -> Result {
        shapes.cap = Cap::None;

        let ccw = fields.bool("ccw")?;
        e.insert((
            Self,
            AttractedInitial { ccw },
            Animation::new(sprites.selene_penumbra.clone_weak(), "anim"),
            AnimationMode::Repeat,
            BaseColor(Color::linear_rgba(1., 2., 24., 1.)),
            DiscComponent::arc(shapes.config(), 12., 0., 0.),
            ShapeMaterial {
                alpha_mode: ShapeAlphaMode::Blend,
                disable_laa: true,
                pipeline: ShapePipelineType::Shape2d,
                canvas: None,
                texture: None,
            },
            ShapeFill {
                color: Color::NONE,
                ty: FillType::Stroke(0., ThicknessType::World),
            },
            DebugRender::none(),
        ))
        .observe(
            |trigger: Trigger<Hurt>,
             mut commands: Commands,
             sprites: Res<Sprites>,
             transforms: Query<&GlobalTransform>,
             level: Query<Entity, (With<Level>, Without<LevelUnload>)>| {
                let Ok(level_entity) = level.single() else { return };
                let Ok(&trns) = transforms.get(trigger.target()) else { return };

                commands.spawn((ChildOf(level_entity), TimeStun::short_instant()));
                commands.spawn((
                    HurtEffect,
                    ChildOf(level_entity),
                    Animation::new(sprites.selene_penumbra_hurt.clone_weak(), "anim"),
                    AnimationSmoothing(Interp::Identity),
                    AnimationHooks::despawn_on_done("anim"),
                    BaseColor(Color::linear_rgb(25., 50., 300.)),
                    Timed::from_anim("anim"),
                    Transform::from(trns),
                    trns,
                ));
            },
        )
        .observe(|trigger: Trigger<TryLaunch>, mut commands: Commands, sprites: Res<Sprites>| {
            commands.entity(trigger.target()).with_children(|children| {
                children.spawn((
                    Transform::from_xyz(0., 0., 0f32.next_up()),
                    Animation::new(sprites.selene_try_launch_front.clone_weak(), "anim"),
                    AnimationHooks::despawn_on_done("anim"),
                    BaseColor(Color::linear_rgb(1., 2., 6.)),
                ));

                children.spawn((
                    Transform::from_xyz(0., 0., 0f32.next_down()),
                    Animation::new(sprites.selene_try_launch_back.clone_weak(), "anim"),
                    AnimationHooks::despawn_on_done("anim"),
                    BaseColor(Color::linear_rgb(1., 2., 6.)),
                ));
            });
        })
        .observe(
            |trigger: Trigger<Launched>, mut commands: Commands, positions: Query<&Position>, sprites: Res<Sprites>| -> Result {
                if let Some(&hurt) = [1, 4, 8].get(trigger.index) {
                    commands.entity(trigger.at).queue(TryHurt::by(trigger.target(), hurt));
                }

                let [&selene_pos, &attractor_pos] = positions.get_many([trigger.target(), trigger.at])?;
                commands
                    .spawn((
                        SlashEffect,
                        Animation::new(sprites.attractor_slash.clone_weak(), "anim"),
                        AnimationSmoothing(Interp::Identity),
                        Timed::from_anim("anim"),
                        BaseColor(Color::linear_rgb(50., 100., 600.)),
                        Transform {
                            translation: attractor_pos.extend(0.),
                            rotation: Quat::from_axis_angle(Vec3::Z, (*attractor_pos - *selene_pos).to_angle()),
                            scale: Vec3::ONE,
                        },
                    ))
                    .observe(Timed::despawn_on_finished);

                Ok(())
            },
        );

        Ok(())
    }
}

pub fn warn_selene_close(
    time: Res<Time>,
    pipeline: Res<SpatialQueryPipeline>,
    mut selene: Query<(Entity, &mut SeleneParry, &GlobalTransform)>,
    layers: Query<&CollisionLayers>,
) {
    let elapsed = time.elapsed();
    for (e, mut parry, &trns) in &mut selene {
        let layer = layers.get(e).copied().unwrap_or_default();
        let mut found = false;
        pipeline.shape_intersections_callback(
            &Collider::circle(parry.radius + parry.warn_radius),
            trns.translation().xy(),
            0.,
            &SpatialQueryFilter::from_mask(layer.filters),
            |hit| {
                let other_layer = layers.get(hit).copied().unwrap_or_default();
                if (other_layer.filters & layer.memberships) != 0 {
                    found = true;
                    false
                } else {
                    true
                }
            },
        );

        if found {
            let just_warned = !std::mem::replace(&mut parry.warned, true);
            parry.warn_time = match parry.warn_time {
                [..] if just_warned => [elapsed; 2],
                [init, ..] => [init, elapsed],
            };
        } else {
            parry.warned = false;
        }
    }
}

pub fn draw_selene_close(mut shapes: ShapePainter, time: Res<Time>, selene: Query<(&SeleneParry, &GlobalTransform)>) {
    const FLASH_DURATION: Duration = Duration::from_millis(125);
    const FADE_DURATION: Duration = Duration::from_millis(500);

    const FLASH_COLOR: Color = Color::linear_rgb(1., 4., 12.);
    const FADE_COLOR: Color = Color::linear_rgba(0., 0.25, 2., 0.5);
    const CLEAR_COLOR: Color = Color::linear_rgba(0., 0., 1., 0.);

    let elapsed = time.elapsed();
    for (&parry, &trns) in &selene {
        let [init, last] = parry.warn_time;
        let flash = (elapsed - init).div_duration_f32(FLASH_DURATION).min(1.);
        let fade = (elapsed - last).div_duration_f32(FADE_DURATION).min(1.);

        shapes.transform = trns.compute_transform();
        shapes.color = FLASH_COLOR.mix(&FADE_COLOR.mix(&CLEAR_COLOR, fade), flash);
        shapes.hollow = true;
        shapes.thickness = 2f32.lerp(1., flash);
        shapes.thickness_type = ThicknessType::World;
        shapes.circle(parry.radius);
    }
}

pub fn color_selene_hurt(mut hurts: Query<(&Timed, &mut BaseColor), With<HurtEffect>>) {
    for (timed, mut color) in &mut hurts {
        **color = Color::linear_rgb(25., 50., 300.)
            .mix(&Color::linear_rgb(0., 1., 12.), timed.frac().pow_out(16))
            .with_alpha(1. - timed.frac().pow_out(4));
    }
}

pub fn color_selene_slash(mut slashes: Query<(&Timed, &mut BaseColor), With<SlashEffect>>) {
    for (timed, mut color) in &mut slashes {
        **color = Color::linear_rgba(50., 100., 600., 1.).mix(&Color::linear_rgba(1., 2., 24., 0.25), timed.frac().pow_out(6));
    }
}

pub fn draw_selene_launch_disc(
    mut selene: Query<(&Rotation, &mut DiscComponent, &mut ShapeFill, &LaunchDurations, Option<&LaunchCharging>), With<SelenePenumbra>>,
) {
    for (&rot, mut disc, mut fill, durations, charging) in &mut selene {
        let rot = rot.as_radians();
        disc.start_angle = rot;

        if let Some(&charging) = charging
            && let Some(&duration) = durations.get(charging.index)
        {
            let len = durations.len();
            let arc_frac = TAU / len as f32;
            let f = charging.time.div_duration_f32(duration);

            disc.cap = Cap::Round;
            *fill = ShapeFill {
                color: match charging.index.checked_sub(1) {
                    None => Color::linear_rgba(1., 2., 4., f * 0.5),
                    Some(0) => Color::linear_rgba(4., 2., 1., 1.),
                    Some(1) => Color::linear_rgba(4., 1., 2., 1.),
                    _ => Color::NONE,
                },
                ty: FillType::Stroke(
                    match charging.index.checked_sub(1) {
                        None => 1.,
                        Some(0) => 1.5,
                        Some(1) => 2.,
                        _ => 0.,
                    },
                    ThicknessType::World,
                ),
            };

            disc.end_angle = f * arc_frac + charging.index as f32 * arc_frac + rot;
        } else {
            disc.cap = Cap::None;
            disc.end_angle = rot;
        }
    }
}

pub fn draw_selene_prediction_trajectory(
    sprites: Res<Sprites>,
    sprite_sections: Res<Assets<SpriteSection>>,
    selene: Query<(&GlobalTransform, &AttractedPrediction, &SpriteDrawer), With<SelenePenumbra>>,
) {
    let Some(ring) = sprite_sections.get(&sprites.ring_1) else { return };

    const SKIP: f32 = 8.;
    for (&trns, prediction, drawer) in &selene {
        let max = prediction.max_distance;
        let mut accum = 0.;
        let mut skip = 0.;

        let Some(mut begin) = prediction.points.first().copied() else { continue };

        for points in prediction.points.windows(2) {
            let [a, b] = *points else { continue };
            let add = (b - a).length();
            accum += add;
            skip += add;

            if skip >= SKIP {
                let count_frac = skip / SKIP;
                let count = count_frac as u32;
                for i in 0..count {
                    let rel = GlobalTransform::from(Transform {
                        translation: begin.lerp(b, i as f32 / count_frac).extend(trns.translation().z),
                        ..default()
                    });

                    let rel = rel.reparented_to(&trns);
                    drawer.draw_at(
                        rel.translation,
                        Rot2::IDENTITY,
                        ring.sprite_with(Color::linear_rgba(1., 2., 4., (1. - accum / max) * 0.75), None, default()),
                    );
                }

                begin = begin.lerp(b, count as f32 / count_frac);
                skip %= SKIP;
            }
        }
    }
}
