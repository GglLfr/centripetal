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
use fastrand::Rng;
use leafwing_input_manager::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    PIXELS_PER_UNIT, SaveApp, Sprites,
    graphics::{Animation, EntityColor, OnAnimateDone, SpriteDrawer, SpriteSection},
    logic::{
        CameraTarget, Fields, FromLevel, InGameState, LevelApp, LevelEntities, LevelUnload, OnTimeFinished, Timed,
        entities::penumbra::AttractedAction,
        levels::{disable, enable, in_level},
    },
    math::{FloatExt, RngExt},
};

#[derive(Debug, Copy, Clone, Default, Resource, TypePath, Serialize, Deserialize, Deref, DerefMut)]
pub struct IntroShown(pub bool);

#[derive(Debug, Copy, Clone, Component)]
pub enum State {
    Init {
        target_pos: Vec2,
    },
    Begin {
        target_pos: Vec2,
        started: Duration,
        done: bool,
    },
    TutorialHover,
    TutorialAccelerate,
}

impl FromLevel for State {
    type Param = (SRes<IntroShown>, SQuery<Read<Transform>>);
    type Data = Read<LevelEntities>;

    fn from_level(
        mut e: EntityCommands,
        _: &Fields,
        (cutscene_shown, transforms): SystemParamItem<Self::Param>,
        entities: QueryItem<Self::Data>,
    ) -> Result {
        if !**cutscene_shown {
            let mut commands = e.commands();
            for iid in [SELENE, ATTRACTOR, RINGS[0], RINGS[1], HOVER_TARGET] {
                commands.get_entity(entities.get(iid)?)?.queue(disable);
            }

            let [&selene_trns, &attractor_trns] = transforms.get_many([entities.get(SELENE)?, entities.get(ATTRACTOR)?])?;
            let target_pos = GlobalTransform::from(selene_trns)
                .reparented_to(&GlobalTransform::from(attractor_trns))
                .translation
                .truncate();

            e.insert(Self::Init { target_pos });
            debug!("Loaded left-wing side Penumbra level (+ intro cutscene)!");
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

fn draw_attractor_spawn_effect(
    sprites: Res<Sprites>,
    sprite_sections: Res<Assets<SpriteSection>>,
    effects: Query<(Entity, &AttractorSpawnEffect, &SpriteDrawer, &Timed)>,
) {
    let Some(ring_6) = sprite_sections.get(&sprites.ring_6) else { return };
    let Some(ring_8) = sprite_sections.get(&sprites.ring_8) else { return };

    let rings = [ring_6, ring_8];
    for (e, effect, drawer, &timed) in &effects {
        let mut rng = Rng::with_seed(e.to_bits());
        let f = timed.frac();

        let mut layer = -1f32;
        for (angle, vec) in
            rng.fork()
                .len_vectors(40, 0., 2. * PI, 5. * PIXELS_PER_UNIT as f32, 10. * PIXELS_PER_UNIT as f32)
        {
            let ring = rings[rng.usize(0..rings.len())];
            let f_scl = f.threshold(0., rng.f32_within(0.65, 1.));

            let green = rng.f32_within(1., 2.);
            let blue = rng.f32_within(12., 24.);
            let alpha = rng.f32_within(0.5, 1.);

            let rotate = f_scl.threshold(0.4, 0.9).pow_in(2);
            let proceed = f_scl.threshold(0.4, 1.);
            let width = ring.size.x + (1. - f_scl.slope(0.5)).pow_in(6) * ring.size.x * 2.;

            drawer.draw_at(
                (vec * f.pow_out(5)).lerp(effect.target_pos, proceed.pow_in(6)).extend(layer),
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

pub const SELENE: Uuid = uuid!("332e5310-3740-11f0-b0d1-4b444b848a1e");
pub const ATTRACTOR: Uuid = uuid!("8226eab0-3740-11f0-b0d1-31c3cf318fb2");
pub const RINGS: [Uuid; 2] = [
    uuid!("483defc0-3740-11f0-bea9-1bca02df9366"),
    uuid!("516847d0-3740-11f0-bea9-db42cbfffb80"),
];
pub const HOVER_TARGET: Uuid = uuid!("ddc89020-3740-11f0-bea9-17dccf039850");

pub const SPAWN_ATTRACTOR_DURATION: Duration = Duration::from_secs(2);

pub fn update(
    mut commands: Commands,
    time: Res<Time>,
    sprite_sheets: Res<Sprites>,
    level: Single<(&mut State, &LevelEntities), Without<LevelUnload>>,
) -> Result {
    let now = time.elapsed();
    let (mut level, entities) = level.into_inner();

    let selene = entities.get(SELENE)?;
    let attractor = entities.get(ATTRACTOR)?;
    let hover_target = entities.get(HOVER_TARGET)?;

    match *level {
        State::Init { target_pos } => {
            *level = State::Begin {
                target_pos,
                started: now,
                done: false,
            }
        }
        State::Begin {
            target_pos,
            started,
            ref mut done,
        } => {
            if now - started >= SPAWN_ATTRACTOR_DURATION && !std::mem::replace(done, true) {
                commands
                    .get_entity(attractor)?
                    .queue(enable)
                    .insert(CameraTarget)
                    .with_children(move |children| {
                        children
                            .spawn((
                                Transform::from_xyz(0., 0., 1.),
                                Animation::new(sprite_sheets.grand_attractor_spawned.clone(), "anim"),
                                EntityColor(Color::linear_rgba(1., 2., 24., 1.)),
                            ))
                            .observe(|trigger: Trigger<OnAnimateDone>, mut commands: Commands| {
                                commands.entity(trigger.target()).despawn();
                            });

                        children
                            .spawn(AttractorSpawnEffect { target_pos })
                            .observe(Timed::despawn_on_finished)
                            .observe(
                                move |_: Trigger<OnTimeFinished>,
                                      mut commands: Commands,
                                      level: Single<&mut State, Without<LevelUnload>>|
                                      -> Result {
                                    commands.get_entity(attractor)?.remove::<CameraTarget>();

                                    commands
                                        .get_entity(hover_target)?
                                        .queue(enable)
                                        .insert(CollisionEventsEnabled)
                                        .observe(
                                            move |trigger: Trigger<OnCollisionStart>,
                                                  mut commands: Commands,
                                                  level: Single<&mut State, Without<LevelUnload>>,
                                                  mut state: Query<&mut ActionState<AttractedAction>>|
                                                  -> Result {
                                                if trigger.body.is_some_and(|body| body == selene) {
                                                    commands.get_entity(trigger.target())?.despawn();

                                                    // Enable accelerating.
                                                    *level.into_inner() = State::TutorialAccelerate;
                                                    state.get_mut(selene)?.enable_action(&AttractedAction::Prograde);
                                                }

                                                Ok(())
                                            },
                                        );

                                    commands
                                        .get_entity(selene)?
                                        .queue(enable)
                                        // Can't use `Queue`s here because it's still `Disabled`.
                                        .queue(|mut e: EntityWorldMut| -> Result {
                                            let mut action = e
                                                .get_mut::<ActionState<AttractedAction>>()
                                                .ok_or("`ActionState<AttractedAction>` not found")?;

                                            // Only `Hover` is enabled initially.
                                            action.disable_action(&AttractedAction::Prograde);
                                            action.disable_action(&AttractedAction::Launch);
                                            action.disable_action(&AttractedAction::Parry);
                                            Ok(())
                                        });

                                    *level.into_inner() = State::TutorialHover;
                                    Ok(())
                                },
                            );
                    });
            }
        }
        State::TutorialHover => {}
        State::TutorialAccelerate => {}
    }

    Ok(())
}

pub(super) fn plugin(app: &mut App) {
    app.register_level::<State>("penumbra_wing_l")
        .add_systems(
            Update,
            (update, draw_attractor_spawn_effect).run_if(in_state(InGameState::Resumed).and(in_level("penumbra_wing_l"))),
        )
        .save_resource_init::<IntroShown>();
}
