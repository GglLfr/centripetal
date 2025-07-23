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

use crate::{
    PIXELS_PER_UNIT, SaveApp, Sprites,
    graphics::{AnimationFrom, AnimationHooks, EntityColor, SpriteDrawer, SpriteSection},
    logic::{
        CameraTarget, Fields, FromLevel, LevelApp, LevelEntities, OnTimeFinished, Timed,
        entities::{
            Health, Killed, NoKillDespawn,
            penumbra::{AttractedAction, AttractedInitial, AttractedPrediction, LaunchAction},
        },
        levels::in_level,
    },
    math::{FloatExt, RngExt},
    resume, suspend, trans_wait,
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
        SQuery<Read<Transform>>,
        SQuery<Read<AttractedInitial>>,
        ShapeCommands<'static, 'static>,
    );
    type Data = Read<LevelEntities>;

    fn from_level(
        mut e: EntityCommands,
        _: &Fields,
        (cutscene_shown, transforms, initials, shapes): SystemParamItem<Self::Param>,
        entities: QueryItem<Self::Data>,
    ) -> Result {
        if !**cutscene_shown {
            let level_entity = e.id();
            let mut commands = e.commands();
            let [selene, attractor, _ring_0, _ring_1, hover_target] =
                [SELENE, ATTRACTOR, RINGS[0], RINGS[1], HOVER_TARGET].map(|iid| {
                    let e = entities.get(iid).unwrap();
                    commands.entity(e).queue(suspend);

                    e
                });

            let initial = initials.get(selene).copied().unwrap_or_default();
            let [&selene_trns, &attractor_trns] = transforms.get_many([selene, attractor])?;

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
                    .truncate();

                move |world: &mut World| -> Result {
                    accept(
                        world
                            .spawn((
                                ChildOf(level_entity),
                                AttractorSpawnEffect { target_pos },
                                effect_trns,
                            ))
                            .observe(Timed::despawn_on_finished)
                            .observe(
                                move |_: Trigger<OnTimeFinished>,
                                      mut commands: Commands|
                                      -> Result {
                                    commands.get_entity(selene)?.queue(resume);
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
                DiscComponent::arc(shapes.config(), 12., 0., 0.),
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
            ));

            e.insert((
                Init,
                StateMachine::default()
                    .trans::<Init, _>(trans_wait(Duration::from_secs(1)), SpawningAttractor)
                    // Attractor spawning effect.
                    .on_enter::<SpawningAttractor>(move |e| {
                        e.with_child((
                            Transform {
                                translation: attractor_trns.translation.with_z(1.),
                                ..attractor_trns
                            },
                            CameraTarget,
                            AnimationFrom::sprite(|sprites| {
                                (sprites.attractor_spawn.clone_weak(), "in")
                            }),
                            AnimationHooks::default()
                                .on_done("in", AnimationHooks::set("out", false))
                                .on_done(
                                    "in",
                                    move |_: In<Entity>, mut commands: Commands| -> Result {
                                        commands.get_entity(level_entity)?.insert(Done::Success);
                                        Ok(())
                                    },
                                )
                                .on_done("out", AnimationHooks::despawn),
                            EntityColor(Color::linear_rgba(1., 2., 24., 1.)),
                        ));
                    })
                    .trans::<SpawningAttractor, _>(done(Some(Done::Success)), SpawningSelene)
                    // Attractor spawned effect, Selene spawning effect.
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
                                    AnimationHooks::default()
                                        .on_done("anim", AnimationHooks::despawn),
                                    EntityColor(Color::linear_rgba(1., 2., 24., 1.)),
                                ));
                            });

                        e.commands().queue(spawn_selene(
                            level_entity,
                            selene,
                            attractor_trns,
                            selene_trns,
                            move |e| {
                                e.observe(
                                    move |_: Trigger<OnTimeFinished>,
                                          mut commands: Commands|
                                          -> Result {
                                        commands.get_entity(level_entity)?.insert(Done::Success);
                                        Ok(())
                                    },
                                );
                                Ok(())
                            },
                        ));
                    })
                    .trans::<SpawningSelene, _>(done(Some(Done::Success)), TutorialMove::default())
                    // Selene spawned effect (TODO), hovering and accelerating tutorial.
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
                            .insert(CollisionEventsEnabled)
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
                                      mut aligned: Query<&mut TutorialMove>|
                                      -> Result {
                                    if trigger.body.is_some_and(|body| body == selene) {
                                        aligned.get_mut(level_entity)?.aligned = false;
                                    }
                                    Ok(())
                                },
                            );
                    })
                    .on_exit::<TutorialMove>(move |e| {
                        e.commands().entity(hover_target).despawn();
                        e.commands().entity(selene).queue(|mut e: EntityWorldMut| {
                            e.get_mut::<ActionState<LaunchAction>>()
                                .expect("`ActionState<LaunchAction>` not found in Selene")
                                .enable_action(&LaunchAction)
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
                    )
                    .trans::<TutorialLaunch, _>(done(Some(Done::Success)), TutorialParry),
            ));
        } else {
            todo!("Revisit this level for the non-intro variant...")
        }

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
    let Some(ring_6) = sprite_sections.get(&sprites.ring_6) else {
        return;
    };
    let Some(ring_8) = sprite_sections.get(&sprites.ring_8) else {
        return;
    };

    let rings = [ring_6, ring_8];
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
