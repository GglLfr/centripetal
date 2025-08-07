use std::{f32::consts::PI, time::Duration};

use avian2d::prelude::*;
use bevy::{
    asset::uuid::{Uuid, uuid},
    ecs::{
        query::QueryItem,
        system::{
            SystemParamItem,
            lifetimeless::{Read, SQuery, SRes},
        },
    },
    prelude::*,
    sprite::Anchor,
    ui::Val::*,
};
use bevy_vector_shapes::{
    prelude::{ShapeCommands, ShapeSpawner},
    render::ShapePipelineType,
    shapes::{
        Cap, DiscComponent, FillType, ShapeAlphaMode, ShapeFill, ShapeMaterial, ThicknessType,
    },
};
use fastrand::Rng;
use leafwing_input_manager::prelude::*;
use seldom_state::prelude::*;
use serde::{Deserialize, Serialize};
use smallvec::smallvec;

use crate::{
    PIXELS_PER_UNIT, SaveApp, Sprites,
    graphics::{
        Animation, AnimationFrom, AnimationHooks, AnimationMode, BaseColor, SpriteDrawer,
        SpriteSection,
    },
    i18n,
    logic::{
        CameraConfines, CameraTarget, Fields, FromLevel, LevelApp, LevelEntities, TimeFinished,
        TimeStun, Timed,
        effects::Ring,
        entities::{
            Health, Killed, NoKillDespawn,
            penumbra::{
                AttractedAction, AttractedInitial, AttractedPrediction, Attractor, HomingTarget,
                LaunchAction, Launched, bullet,
            },
        },
        levels::in_level,
    },
    math::{FloatTransformExt, Interp, RngExt},
    resume, suspend, trans_wait,
    ui::{WorldspaceUi, ui_fade_in, ui_fade_out, ui_hide, widgets},
};

#[derive(
    Debug, Copy, Clone, Default, Resource, TypePath, Serialize, Deserialize, Deref, DerefMut,
)]
struct IntroShown(pub bool);

#[derive(Debug, Copy, Clone, Default)]
struct Instance;

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
struct Init;

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
struct SpawningAttractor;

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
struct SpawningSelene;

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
struct TutorialMove {
    align_time: Duration,
    aligned: bool,
}

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
struct TutorialLaunch;

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
struct TutorialParry;

impl FromLevel for Instance {
    type Param = (
        SRes<IntroShown>,
        SRes<Sprites>,
        SQuery<Read<Transform>>,
        SQuery<Read<AttractedInitial>>,
        SQuery<Read<Attractor>>,
        ShapeCommands<'static, 'static>,
    );
    type Data = Read<LevelEntities>;

    fn from_level(
        mut e: EntityCommands,
        _: &Fields,
        (cutscene_shown, sprites, transforms, initials, attractors, shapes): SystemParamItem<
            Self::Param,
        >,
        entities: QueryItem<Self::Data>,
    ) -> Result {
        if **cutscene_shown {
            //TODO Can this level be revisited without the cutscene?
            return Ok(());
        }

        let level_entity = e.id();
        let mut commands = e.commands();
        let [selene, attractor, ring_0, ring_1, hover_target] =
            [SELENE, ATTRACTOR, RINGS[0], RINGS[1], HOVER_TARGET].map(|iid| {
                let e = entities.get(iid).unwrap();
                commands.entity(e).queue(suspend);

                e
            });

        let initial = initials.get(selene).copied().unwrap_or_default();
        let attractor_radius = attractors.get(attractor)?.radius;
        let [&selene_trns, &attractor_trns] = transforms.get_many([selene, attractor])?;

        #[derive(Debug, Copy, Clone, Default, Event)]
        struct Respawned;

        #[must_use]
        fn spawn_selene(
            level_entity: Entity,
            selene: Entity,
            effect_trns: Transform,
            selene_trns: Transform,
            accept: impl FnOnce(&mut EntityWorldMut) -> Result + 'static + Send,
        ) -> impl Command<Result> {
            let target_pos = GlobalTransform::from(selene_trns)
                .reparented_to(&GlobalTransform::from(effect_trns))
                .translation
                .xy();

            move |world: &mut World| -> Result {
                world
                    .spawn((
                        ChildOf(level_entity),
                        effect_trns,
                        Ring {
                            radius_to: 128.,
                            thickness_from: 2.,
                            colors: smallvec![
                                Color::linear_rgb(1., 2., 6.),
                                Color::linear_rgb(1., 1., 2.)
                            ],
                            radius_interp: Interp::PowOut { exponent: 2 },
                            ..default()
                        },
                        Timed::new(Duration::from_millis(640)),
                    ))
                    .observe(Timed::despawn_on_finished);

                accept(
                    world
                        .spawn((
                            ChildOf(level_entity),
                            AttractorSpawnEffect { target_pos },
                            effect_trns,
                        ))
                        .observe(Timed::despawn_on_finished)
                        .observe(
                            move |_: Trigger<TimeFinished>, mut commands: Commands| -> Result {
                                commands
                                    .get_entity(selene)?
                                    .queue(resume)
                                    .trigger(Respawned);
                                Ok(())
                            },
                        ),
                )
            }
        }

        commands.entity(selene).insert(NoKillDespawn).observe(
            move |trigger: Trigger<Killed>,
                  mut commands: Commands,
                  mut query: Query<(&Transform, &mut AttractedPrediction)>|
                  -> Result {
                let (&trns, mut prediction) = query.get_mut(trigger.target())?;
                prediction.points.clear();

                commands
                    .get_entity(selene)?
                    .insert((
                        selene_trns,
                        initial,
                        LinearVelocity::ZERO,
                        AngularVelocity::ZERO,
                        Health::new(10),
                    ))
                    .queue(suspend);

                commands.queue(spawn_selene(
                    level_entity,
                    selene,
                    trns,
                    selene_trns,
                    |_| Ok(()),
                ));

                Ok(())
            },
        );

        commands.entity(hover_target).insert((
            Collider::circle(8.),
            CollisionEventsEnabled,
            Animation::new(sprites.collectible_32.clone_weak(), "anim"),
            AnimationMode::Repeat,
            BaseColor(Color::linear_rgba(12., 2., 1., 1.)),
            DiscComponent::arc(shapes.config(), 16., 0., 0.),
            ShapeMaterial {
                alpha_mode: ShapeAlphaMode::Blend,
                disable_laa: true,
                pipeline: ShapePipelineType::Shape2d,
                canvas: None,
                texture: None,
            },
            ShapeFill {
                color: Color::linear_rgba(4., 2., 1., 1.),
                ty: FillType::Stroke(1.5, ThicknessType::World),
            },
            Timed::repeat(
                Duration::from_secs(1),
                |In(e): In<Entity>, mut commands: Commands| {
                    commands.spawn((
                        ChildOf(e),
                        Transform::from_xyz(0., 0., -1.),
                        Ring {
                            radius_to: 16.,
                            thickness_from: 3.,
                            colors: smallvec![Color::linear_rgb(4., 2., 1.)],
                            ..default()
                        },
                        Timed::new(Duration::from_millis(750)),
                    ));
                },
            ),
            DebugRender::none(),
        ));

        let mut states = StateMachine::default()
            .trans::<Init, _>(trans_wait(Duration::from_secs(1)), SpawningAttractor);

        // PHASE 1: Attractor spawning effect.
        states = states
            .on_enter::<SpawningAttractor>(move |e| {
                e.remove::<(CameraTarget, CameraConfines)>().with_child((
                    Transform {
                        translation: attractor_trns.translation.with_z(1.),
                        ..attractor_trns
                    },
                    CameraTarget,
                    AnimationFrom::sprite(|sprites| (sprites.attractor_spawn.clone_weak(), "in")),
                    AnimationHooks::default()
                        .on_done("in", AnimationHooks::set("out", false))
                        .on_done(
                            "in",
                            move |_: In<Entity>, mut commands: Commands| -> Result {
                                commands.get_entity(level_entity)?.insert(Done::Success);
                                commands
                                    .spawn((
                                        ChildOf(level_entity),
                                        attractor_trns,
                                        Ring {
                                            radius_from: attractor_radius,
                                            radius_to: attractor_radius + 24.,
                                            thickness_from: 2.,
                                            colors: smallvec![
                                                Color::linear_rgb(1., 2., 6.),
                                                Color::linear_rgb(1., 1., 2.)
                                            ],
                                            radius_interp: Interp::PowIn { exponent: 3 },
                                            ..default()
                                        },
                                        Timed::new(Duration::from_millis(480)),
                                    ))
                                    .observe(Timed::despawn_on_finished);

                                Ok(())
                            },
                        )
                        .on_done("out", AnimationHooks::despawn),
                    BaseColor(Color::linear_rgba(1., 2., 24., 1.)),
                ));
            })
            .trans::<SpawningAttractor, _>(done(None), SpawningSelene);

        // PHASE 2: Attractor spawned effect, Selene spawning effect.
        states = states
            .on_enter::<SpawningSelene>(move |e| {
                e.commands()
                    .entity(attractor)
                    .queue(resume)
                    .insert(CameraTarget)
                    .with_children(move |children| {
                        children.spawn((
                            Transform::from_xyz(0., 0., 1.),
                            AnimationFrom::sprite(|sprites| {
                                (sprites.grand_attractor_spawned.clone_weak(), "anim")
                            }),
                            AnimationHooks::despawn_on_done("anim"),
                            BaseColor(Color::linear_rgba(1., 2., 24., 1.)),
                        ));
                    });

                e.commands().queue(spawn_selene(
                    level_entity,
                    selene,
                    attractor_trns,
                    selene_trns,
                    move |e| {
                        e.observe(
                            move |_: Trigger<TimeFinished>, mut commands: Commands| -> Result {
                                commands.get_entity(level_entity)?.insert(Done::Success);
                                Ok(())
                            },
                        );
                        Ok(())
                    },
                ));
            })
            .trans::<SpawningSelene, _>(done(None), TutorialMove::default());

        // PHASE 3 (moving tutorial): Selene spawned effect (TODO), hovering and accelerating tutorial.
        let ui_selene_hover = commands
            .spawn((
                Node {
                    display: Display::Grid,
                    grid_template_columns: vec![RepeatedGridTrack::min_content(2)],
                    row_gap: Px(3.),
                    column_gap: Px(9.),
                    ..default()
                },
                WorldspaceUi {
                    target: selene,
                    offset: vec2(0., -16.),
                    anchor: vec2(0.5, 1.),
                },
                children![
                    (
                        Node {
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::End,
                            ..default()
                        },
                        children![(
                            widgets::icon(),
                            children![(
                                widgets::keyboard_binding(|binds| binds.attracted_hover[0]),
                                TextColor(Color::BLACK),
                            )],
                        )],
                    ),
                    (
                        Node::default(),
                        children![(
                            widgets::shadow_bg(),
                            widgets::text(i18n!("tutorial.hover.descend")),
                            TextLayout::new(JustifyText::Left, LineBreak::NoWrap),
                        )],
                    ),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::End,
                            ..default()
                        },
                        children![(
                            widgets::icon(),
                            children![(
                                widgets::keyboard_binding(|binds| binds.attracted_hover[1]),
                                TextColor(Color::BLACK),
                            )],
                        )],
                    ),
                    (
                        Node::default(),
                        children![(
                            widgets::shadow_bg(),
                            widgets::text(i18n!("tutorial.hover.ascend")),
                            TextLayout::new(JustifyText::Left, LineBreak::NoWrap),
                        )],
                    ),
                ],
            ))
            .queue(ui_hide)
            .id();

        let ui_selene_accel = commands
            .spawn((
                Node {
                    display: Display::Grid,
                    grid_template_columns: vec![RepeatedGridTrack::min_content(2)],
                    row_gap: Px(3.),
                    column_gap: Px(9.),
                    ..default()
                },
                WorldspaceUi {
                    target: selene,
                    offset: vec2(0., -16.),
                    anchor: vec2(0.5, 1.),
                },
                children![
                    (
                        Node {
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::End,
                            ..default()
                        },
                        children![(
                            widgets::icon(),
                            children![(
                                widgets::keyboard_binding(|binds| binds.attracted_accel[0]),
                                TextColor(Color::BLACK),
                            )],
                        )],
                    ),
                    (
                        Node::default(),
                        children![(
                            widgets::shadow_bg(),
                            widgets::text(i18n!("tutorial.hover.retrograde")),
                            TextLayout::new(JustifyText::Left, LineBreak::NoWrap),
                        )],
                    ),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::End,
                            ..default()
                        },
                        children![(
                            widgets::icon(),
                            children![(
                                widgets::keyboard_binding(|binds| binds.attracted_accel[1]),
                                TextColor(Color::BLACK),
                            )],
                        )],
                    ),
                    (
                        Node::default(),
                        children![(
                            widgets::shadow_bg(),
                            widgets::text(i18n!("tutorial.hover.prograde")),
                            TextLayout::new(JustifyText::Left, LineBreak::NoWrap),
                        )],
                    ),
                ],
            ))
            .queue(ui_hide)
            .id();

        #[derive(Debug, Resource, Deref, DerefMut)]
        struct ShownTutorialUi(Entity);

        commands.insert_resource(ShownTutorialUi(ui_selene_hover));
        commands
            .entity(selene)
            .observe(
                move |_: Trigger<Respawned>, mut commands: Commands, ui: Res<ShownTutorialUi>| {
                    if let Ok(mut e) = commands.get_entity(**ui) {
                        e.queue(ui_fade_in);
                    }
                },
            )
            .observe(
                move |_: Trigger<Killed>, mut commands: Commands, ui: Res<ShownTutorialUi>| {
                    if let Ok(mut e) = commands.get_entity(**ui) {
                        e.queue(ui_fade_out);
                    }
                },
            );

        states = states
            .on_enter::<TutorialMove>(move |e| {
                e.commands().entity(attractor).remove::<CameraTarget>();
                e.commands()
                    .entity(selene)
                    .queue(|mut e: EntityWorldMut| -> Result {
                        // Only `Hover` and `Accel` are enabled initially.
                        e.get_mut::<ActionState<AttractedAction>>()
                            .ok_or("`ActionState<AttractedAction>` not found")?
                            .disable_action(&AttractedAction::Parry);

                        e.get_mut::<ActionState<LaunchAction>>()
                            .ok_or("`ActionState<LaunchAction>` not found")?
                            .disable_action(&LaunchAction);

                        Ok(())
                    });

                e.commands()
                    .entity(hover_target)
                    .queue(resume)
                    .observe(
                        move |trigger: Trigger<OnCollisionStart>,
                              mut aligned: Query<&mut TutorialMove>|
                              -> Result {
                            if trigger.body.is_some_and(|body| body == selene) {
                                aligned.get_mut(level_entity)?.aligned = true;
                            }

                            Ok(())
                        },
                    )
                    .observe(
                        move |trigger: Trigger<OnCollisionEnd>,
                              mut commands: Commands,
                              mut shown_ui: ResMut<ShownTutorialUi>,
                              mut aligned: Query<&mut TutorialMove>|
                              -> Result {
                            let mut aligned = aligned.get_mut(level_entity)?;
                            if trigger.body.is_some_and(|body| body == selene)
                                && std::mem::replace(&mut aligned.aligned, false)
                                && aligned.align_time >= TUTORIAL_MOVE_ALIGN_HELP
                                && commands
                                    .get_entity(ui_selene_accel)
                                    .map(|mut e| {
                                        e.queue(ui_fade_in);
                                    })
                                    .is_ok()
                                && let Ok(mut ui) = commands
                                    .get_entity(std::mem::replace(&mut **shown_ui, ui_selene_accel))
                            {
                                ui.queue(ui_fade_out);
                            }

                            Ok(())
                        },
                    );
            })
            .on_exit::<TutorialMove>(move |e| {
                e.commands().entity(hover_target).despawn();
                e.commands()
                    .entity(selene)
                    .queue(|mut e: EntityWorldMut| -> Result {
                        e.get_mut::<ActionState<LaunchAction>>()
                            .ok_or("`ActionState<LaunchAction>` not found in Selene")?
                            .enable_action(&LaunchAction);
                        Ok(())
                    });
            })
            .trans::<TutorialMove, _>(
                |In(level_entity): In<Entity>, aligned: Query<&TutorialMove>| {
                    aligned
                        .get(level_entity)
                        .expect("`TutorialMove` in level entity")
                        .align_time
                        >= TUTORIAL_MOVE_ALIGN_DURATION
                },
                TutorialLaunch,
            );

        // PHASE 4 (parrying tutorial)
        states = states
            .on_enter::<TutorialLaunch>(move |e| {
                e.commands().entity(selene).observe(
                    move |trigger: Trigger<Launched>,
                            mut commands: Commands,
                            positions: Query<&Position>| {
                        if trigger.at == attractor
                            && let Ok([&pos, &attractor_pos]) =
                                positions.get_many([trigger.target(), trigger.at])
                        {
                            commands.entity(trigger.observer()).despawn();
                            commands
                                .spawn((ChildOf(level_entity), TimeStun::long_smooth()));

                            #[derive(Event)]
                            struct Hit;

                            for i in 0..3 {
                                let Rotation { cos, sin } = (*pos - *attractor_pos)
                                    .try_normalize()
                                    .map(|Vec2 { x: cos, y: sin }| Rotation { cos, sin })
                                    .unwrap_or_default()
                                    * Rotation::radians(2. * PI * i as f32 / 3.);

                                commands.spawn(Timed::run(
                                    Duration::from_millis((i as u64 + 1) * 150),
                                    move |_: In<Entity>, mut commands: Commands| {
                                        let bullet = commands.spawn((
                                            bullet::spiky(level_entity),
                                            HomingTarget(selene),
                                            LinearVelocity(vec2(cos * 156., sin * 156.)),
                                            attractor_pos,
                                            Rotation { cos, sin },
                                        )).observe(move |trigger: Trigger<OnCollisionStart>, mut commands: Commands| {
                                            if trigger.body.is_some_and(|body| body == selene) {
                                                commands.trigger(Hit);
                                            }
                                        }).id();

                                        commands.spawn((
                                            ChildOf(level_entity),
                                            Observer::new(move |trigger: Trigger<Hit>, mut commands: Commands| {
                                                if let Ok(mut e) = commands.get_entity(bullet) {
                                                    e.remove::<HomingTarget>();
                                                    commands.entity(trigger.observer()).despawn();
                                                }
                                            }),
                                        ));
                                    },
                                ));
                            }

                            commands.spawn(Timed::run(
                                Duration::from_millis(750),
                                move |_: In<Entity>, mut commands: Commands| -> Result {
                                    commands.get_entity(ring_0)?.queue(resume);
                                    commands.get_entity(ring_1)?.queue(resume);
                                    Ok(())
                                },
                            ));
                        }
                    },
                );
            })
            .trans::<TutorialLaunch, _>(done(None), TutorialParry);

        e.insert((
            Init,
            CameraTarget,
            CameraConfines::Fixed(attractor_trns.translation.xy()),
            states,
        ));

        Ok(())
    }
}

#[derive(Debug, Clone, Component, Default)]
#[require(SpriteDrawer, Timed::new(Duration::from_millis(2500)))]
struct AttractorSpawnEffect {
    target_pos: Vec2,
}

fn update_tutorial_move_aligned(
    time: Res<Time>,
    mut aligned: Query<(&LevelEntities, &mut TutorialMove)>,
    mut target: Query<&mut DiscComponent>,
) {
    let delta = time.delta();
    let Ok((entities, mut align)) = aligned.single_mut() else {
        return;
    };

    align.align_time = if align.aligned {
        (align.align_time + delta).min(TUTORIAL_MOVE_ALIGN_DURATION)
    } else {
        align.align_time.saturating_sub(delta)
    };

    let Some(mut component) = entities
        .get(HOVER_TARGET)
        .ok()
        .and_then(|e| target.get_mut(e).ok())
    else {
        return;
    };

    component.end_angle = 2.
        * PI
        * align
            .align_time
            .div_duration_f32(TUTORIAL_MOVE_ALIGN_DURATION);

    component.cap = if align.align_time > Duration::ZERO {
        Cap::Round
    } else {
        Cap::None
    };
}

fn draw_attractor_spawn_effect(
    sprites: Res<Sprites>,
    sprite_sections: Res<Assets<SpriteSection>>,
    effects: Query<(Entity, &AttractorSpawnEffect, &SpriteDrawer, &Timed)>,
) {
    let rings @ [Some(..), Some(..), Some(..), Some(..), Some(..)] = [
        sprite_sections.get(&sprites.ring_2),
        sprite_sections.get(&sprites.ring_3),
        sprite_sections.get(&sprites.ring_4),
        sprite_sections.get(&sprites.ring_6),
        sprite_sections.get(&sprites.ring_8),
    ] else {
        return;
    };
    let rings = rings.map(Option::unwrap);
    for (e, effect, drawer, &timed) in &effects {
        let mut rng = Rng::with_seed(e.to_bits());
        let f = timed.frac();

        let mut layer = -1f32;
        for (angle, vec) in rng.fork().len_vectors(
            40,
            0.,
            2. * PI,
            5. * PIXELS_PER_UNIT as f32,
            10. * PIXELS_PER_UNIT as f32,
        ) {
            let ring = rings[rng.usize(0..rings.len())];
            let f_scl = f.threshold(0., rng.f32_within(0.75, 1.));

            let green = rng.f32_within(1., 2.);
            let blue = rng.f32_within(12., 24.);
            let alpha = rng.f32_within(0.5, 1.);

            let rotate = f_scl.threshold(0.4, 0.9).pow_in(2);
            let proceed = f_scl.threshold(0.25, 1.);
            let width = ring.size.x + (1. - f_scl.slope(0.5)).pow_in(6) * ring.size.x * 1.5;

            drawer.draw_at(
                (vec * f.pow_out(5))
                    .lerp(effect.target_pos, proceed.pow_in(6))
                    .extend(layer),
                angle.slerp(Rot2::radians((effect.target_pos - vec).to_angle()), rotate),
                ring.sprite_with(
                    Color::linear_rgba(1., green, blue, alpha * (1. - proceed.pow_in(7))),
                    vec2(width, ring.size.y),
                    Anchor::CenterRight,
                ),
            );

            layer = layer.next_down();
        }
    }
}

const SELENE: Uuid = uuid!("332e5310-3740-11f0-b0d1-4b444b848a1e");
const ATTRACTOR: Uuid = uuid!("8226eab0-3740-11f0-b0d1-31c3cf318fb2");
const RINGS: [Uuid; 2] = [
    uuid!("483defc0-3740-11f0-bea9-1bca02df9366"),
    uuid!("516847d0-3740-11f0-bea9-db42cbfffb80"),
];
const HOVER_TARGET: Uuid = uuid!("ddc89020-3740-11f0-bea9-17dccf039850");

const TUTORIAL_MOVE_ALIGN_HELP: Duration = Duration::from_millis(500);
const TUTORIAL_MOVE_ALIGN_DURATION: Duration = Duration::from_secs(5);

pub(super) fn plugin(app: &mut App) {
    app.register_level::<Instance>("penumbra_wing_l")
        .add_systems(
            Update,
            (draw_attractor_spawn_effect, update_tutorial_move_aligned)
                .run_if(in_level("penumbra_wing_l")),
        )
        .save_resource_init::<IntroShown>();
}
