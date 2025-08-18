use std::f32::consts::TAU;

use crate::{
    i18n,
    logic::{
        Timed,
        entities::{
            Killed, ParryCollider, TryHurt,
            penumbra::{HomingTarget, bullet},
        },
        levels::penumbra_wing_l::{Instance, SeleneUi, p4_tutorial_launch},
    },
    math::RngExt,
    prelude::*,
    ui::{BottomDialog, WorldspaceUi, ui_fade_in, ui_fade_out, ui_hide, widgets},
};

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
pub struct TutorialParry;

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
struct ParryOne;

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
struct ParryMultiple;

pub fn init(
    InRef(&Instance {
        level_entity,
        selene,
        attractor,
        ..
    }): InRef<Instance>,
    mut commands: Commands,
) -> Result {
    let ui_selene_parry = commands
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
                    children![(widgets::icon(), children![(
                        widgets::keyboard_binding(|binds| binds.attracted_parry),
                        TextColor(Color::BLACK),
                    )],)],
                ),
                (Node::default(), children![(
                    widgets::shadow_bg(),
                    widgets::text(i18n!("tutorial.parry.parry")),
                    TextLayout::new(JustifyText::Left, LineBreak::NoWrap),
                )],),
            ],
        ))
        .queue(ui_hide)
        .id();

    // Entry point.
    commands.entity(level_entity).observe(
        move |trigger: Trigger<OnRemove, p4_tutorial_launch::TutorialLaunch>,
              launches: Query<&p4_tutorial_launch::TutorialLaunch>,
              mut commands: Commands|
              -> Result {
            commands.entity(trigger.observer()).despawn();
            commands.entity(level_entity).insert(TutorialParry);

            // If we got the "special" outcome (obtained by not getting hit, which is by parrying), skip the
            // parrying tutorial section completely.
            let &launch = launches.get(level_entity)?;
            let (initial, skip) = if let p4_tutorial_launch::TutorialLaunch::Normal = launch {
                (i18n!("tutorial.parry.enter.normal"), false)
            } else {
                (i18n!("tutorial.parry.enter.special"), true)
            };

            // Some intermission texts.
            commands.queue(BottomDialog::show(
                None,
                initial,
                BottomDialog::show_next_after(
                    Duration::from_secs(2),
                    i18n!("tutorial.parry.recognition"),
                    move |_: In<Entity>, mut commands: Commands| {
                        commands.spawn((
                            ChildOf(level_entity),
                            Timed::run(Duration::from_secs(2), move |mut commands: Commands| {
                                // If the player already knows how to parry, skip it altogether.
                                // This is intended as a speedrun mechanic.
                                if skip {
                                    commands.entity(level_entity).remove::<TutorialParry>();
                                } else {
                                    commands.entity(level_entity).insert(ParryOne);
                                }
                            }),
                        ));
                    },
                ),
            ));

            Ok(())
        },
    );

    // 1: Parry one bullet.
    commands.entity(level_entity).observe(
        move |trigger: Trigger<OnInsert, ParryOne>, mut commands: Commands, mut ui: ResMut<SeleneUi>| -> Result {
            #[derive(Debug, Copy, Clone, Event)]
            struct FireMultiple(u32);
            #[derive(Debug, Copy, Clone, Component)]
            struct Parried(bool);

            fn spawn_bullet(
                In([level_entity, selene, attractor]): In<[Entity; 3]>,
                mut commands: Commands,
                positions: Query<&Position>,
            ) -> Result<Entity> {
                let &attractor_pos = positions.get(attractor)?;
                let selene_pos = positions.get(selene).ok().copied();

                let bullet = commands.spawn_empty().id();
                let angle = if let Some(vec) = selene_pos.and_then(|selene_pos| (*selene_pos - *attractor_pos).try_normalize()) {
                    vec
                } else {
                    Vec2::from_angle(Rng::with_seed(bullet.to_bits()).f32_within(0., TAU))
                };

                commands.spawn((
                    ChildOf(level_entity),
                    Observer::new(move |mut trigger: Trigger<TryHurt>, mut commands: Commands| {
                        if trigger.by == bullet {
                            commands.entity(trigger.observer()).despawn();
                            trigger.stop();
                        }
                    })
                    .with_entity(selene),
                ));

                commands
                    .entity(bullet)
                    .insert((
                        bullet::spiky(level_entity),
                        HomingTarget(selene),
                        LinearVelocity(angle * 96.),
                        attractor_pos,
                        Rotation { cos: angle.x, sin: angle.y },
                        Parried(false),
                    ))
                    .observe(
                        |trigger: Trigger<OnCollisionStart>, mut query: Query<&mut Parried>, parry: Query<(), With<ParryCollider>>| {
                            if trigger.body.is_some_and(|body| parry.contains(body))
                                && let Ok(mut parried) = query.get_mut(trigger.target())
                            {
                                parried.0 = true;
                            }
                        },
                    );

                Ok(bullet)
            }

            // This observer calls `spawn_bullet` after optionally displaying a dialog.
            commands.spawn((
                ChildOf(level_entity),
                Observer::new(move |trigger: Trigger<FireMultiple>, world: &mut World| -> Result {
                    let bullet = world.run_system_cached_with(spawn_bullet, [level_entity, selene, attractor])??;

                    let num_fired = trigger.0;
                    world.entity_mut(bullet).observe(
                        move |trigger: Trigger<OnRemove, RigidBody>,
                              mut commands: Commands,
                              query: Query<&Parried>,
                              mut ui: ResMut<SeleneUi>|
                              -> Result {
                            let &Parried(parried) = query.get(trigger.target())?;
                            if !parried {
                                match num_fired {
                                    1..=2 => commands.queue(BottomDialog::show(
                                        None,
                                        i18n!(format!("tutorial.parry.fail.1-{num_fired}")),
                                        move |In(e): In<Entity>, mut commands: Commands| {
                                            commands.spawn((
                                                ChildOf(level_entity),
                                                Timed::run(Duration::from_secs(2), move |world: &mut World| -> Result {
                                                    world.trigger(FireMultiple(num_fired + 1));
                                                    BottomDialog::hide(e).apply(world)
                                                }),
                                            ));
                                        },
                                    )),
                                    _ => {
                                        commands.spawn((
                                            ChildOf(level_entity),
                                            Timed::run(Duration::from_secs(1), move |world: &mut World| {
                                                world.trigger(FireMultiple(3));
                                            }),
                                        ));
                                    }
                                }
                            } else {
                                commands.entity(trigger.observer()).despawn();
                                if let Some(ui) = ui.take_if(|&mut ui| ui == ui_selene_parry) {
                                    commands.entity(ui).queue(ui_fade_out);
                                }

                                commands.queue(BottomDialog::show(
                                    None,
                                    i18n!("tutorial.parry.success.1.1"),
                                    BottomDialog::show_next_after(
                                        Duration::from_secs(2),
                                        i18n!("tutorial.parry.success.1.2"),
                                        move |In(e): In<Entity>, mut commands: Commands| {
                                            commands.spawn((
                                                ChildOf(level_entity),
                                                Timed::run(Duration::from_secs(2), move |world: &mut World| -> Result {
                                                    world.get_entity_mut(level_entity)?.remove::<ParryOne>().insert(ParryMultiple);
                                                    BottomDialog::hide(e).apply(world)
                                                }),
                                            ));
                                        },
                                    ),
                                ));
                            }

                            Ok(())
                        },
                    );

                    Ok(())
                }),
            ));

            // Show some initial dialog...
            commands.entity(trigger.observer()).despawn();
            commands.queue(BottomDialog::show(
                None,
                i18n!("tutorial.parry.aid"),
                move |In(e): In<Entity>, mut commands: Commands| {
                    commands.spawn((
                        ChildOf(level_entity),
                        Timed::run(Duration::from_secs(2), move |world: &mut World| -> Result {
                            // ...then start firing the bullet.
                            BottomDialog::hide(e).apply(world)?;
                            world.trigger(FireMultiple(1));

                            Ok(())
                        }),
                    ));
                },
            ));

            commands.entity(ui_selene_parry).queue(ui_fade_in);
            if let Some(ui) = ui.replace(ui_selene_parry)
                && let Ok(mut ui) = commands.get_entity(ui)
            {
                ui.queue(ui_fade_out);
            }

            Ok(())
        },
    );

    // 2: Parry multiple bullets.
    commands
        .entity(level_entity)
        .observe(move |trigger: Trigger<OnInsert, ParryMultiple>, mut commands: Commands| {
            #[derive(Debug, Copy, Clone, Event)]
            struct FireMultiple(u32);

            fn spawn_bullet(
                In([level_entity, selene, attractor]): In<[Entity; 3]>,
                mut commands: Commands,
                positions: Query<&Position>,
                mut rng: Local<Rng>,
            ) -> Result<[Entity; 3]> {
                let &attractor_pos = positions.get(attractor)?;
                let selene_pos = positions.get(selene).ok().copied();

                let mut base_angle = if let Some(vec) = selene_pos.and_then(|selene_pos| (*selene_pos - *attractor_pos).try_normalize()) {
                    Rotation { cos: vec.x, sin: vec.y }
                } else {
                    Rotation::radians(rng.f32_within(0., TAU))
                };

                let mut bullets = [Entity::PLACEHOLDER; 3];
                let incr = Rotation::radians(TAU / bullets.len() as f32);

                for (i, e) in bullets.iter_mut().enumerate() {
                    *e = commands.spawn_empty().id();
                    let bullet = *e;

                    let angle = base_angle;
                    base_angle *= incr;

                    commands.spawn((
                        ChildOf(level_entity),
                        Timed::run(Duration::from_millis(i as u64 * 250), move |mut commands: Commands| {
                            commands.entity(bullet).insert((
                                bullet::spiky(level_entity),
                                HomingTarget(selene),
                                LinearVelocity(vec2(angle.cos, angle.sin) * 96.),
                                attractor_pos,
                                angle,
                                //Parried(false),
                            ));
                        }),
                    ));
                }

                commands.spawn((
                    ChildOf(level_entity),
                    Observer::new(move |mut trigger: Trigger<TryHurt>, mut commands: Commands| {
                        commands.entity(trigger.observer()).despawn();
                        if bullets.contains(&trigger.by) {
                            trigger.stop();
                        }
                    })
                    .with_entity(selene),
                ));

                Ok(bullets)
            }

            commands.spawn((
                ChildOf(level_entity),
                Observer::new(move |trigger: Trigger<FireMultiple>, world: &mut World| -> Result {
                    #[derive(Debug, Copy, Clone, Component)]
                    struct ParryCount(usize);

                    let bullets = world.run_system_cached_with(spawn_bullet, [level_entity, selene, attractor])??;
                    let mut hit_observer = Observer::new(
                        move |trigger: Trigger<Killed>, mut commands: Commands, mut query: Query<&mut ParryCount>| -> Result {
                            if trigger.by == selene {
                                let mut count = query.get_mut(trigger.observer())?;
                                count.0 += 1;

                                if count.0 == bullets.len() {
                                    commands.entity(trigger.observer()).despawn();
                                }
                            } else {
                                for b in bullets {
                                    commands.entity(b).try_despawn();
                                }
                                commands.entity(trigger.observer()).despawn();
                            }

                            Ok(())
                        },
                    );

                    for e in bullets {
                        hit_observer.watch_entity(e);
                    }

                    let num_fired = trigger.0;
                    world.spawn((ChildOf(level_entity), ParryCount(0), hit_observer)).observe(
                        move |trigger: Trigger<OnRemove, ParryCount>, mut commands: Commands, query: Query<&ParryCount>| -> Result {
                            let &ParryCount(count) = query.get(trigger.target())?;
                            if count != bullets.len() {
                                match num_fired {
                                    1 => commands.queue(BottomDialog::show(
                                        None,
                                        i18n!("tutorial.parry.fail.2-1.1"),
                                        BottomDialog::show_next_after(
                                            Duration::from_secs(2),
                                            i18n!("tutorial.parry.fail.2-1.2"),
                                            BottomDialog::show_next_after(
                                                Duration::from_secs(2),
                                                i18n!("tutorial.parry.fail.2-1.3"),
                                                move |In(e): In<Entity>, mut commands: Commands| {
                                                    commands.spawn((
                                                        ChildOf(level_entity),
                                                        Timed::run(Duration::from_secs(2), move |world: &mut World| -> Result {
                                                            world.trigger(FireMultiple(num_fired + 1));
                                                            BottomDialog::hide(e).apply(world)
                                                        }),
                                                    ));
                                                },
                                            ),
                                        ),
                                    )),
                                    2 => commands.queue(BottomDialog::show(
                                        None,
                                        i18n!("tutorial.parry.fail.2-2"),
                                        move |In(e): In<Entity>, mut commands: Commands| {
                                            commands.spawn((
                                                ChildOf(level_entity),
                                                Timed::run(Duration::from_secs(2), move |world: &mut World| -> Result {
                                                    world.trigger(FireMultiple(num_fired + 1));
                                                    BottomDialog::hide(e).apply(world)
                                                }),
                                            ));
                                        },
                                    )),
                                    _ => {
                                        commands.spawn((
                                            ChildOf(level_entity),
                                            Timed::run(Duration::from_secs(1), move |world: &mut World| {
                                                world.trigger_targets(FireMultiple(3), level_entity);
                                            }),
                                        ));
                                    }
                                }
                            } else {
                                commands.queue(BottomDialog::show(
                                    None,
                                    i18n!("tutorial.parry.success.2.1"),
                                    BottomDialog::show_next_after(
                                        Duration::from_secs(2),
                                        i18n!("tutorial.parry.success.2.2"),
                                        move |In(e): In<Entity>, mut commands: Commands| {
                                            commands.spawn((
                                                ChildOf(level_entity),
                                                Timed::run(Duration::from_secs(2), move |world: &mut World| -> Result {
                                                    world.get_entity_mut(level_entity)?.remove::<(ParryMultiple, TutorialParry)>();
                                                    BottomDialog::hide(e).apply(world)
                                                }),
                                            ));
                                        },
                                    ),
                                ));
                            }

                            Ok(())
                        },
                    );

                    Ok(())
                }),
            ));

            // Start firing the bullets immediately. The continuation dialog was done previously.
            commands.entity(trigger.observer()).despawn();
            commands.trigger(FireMultiple(1));
        });

    Ok(())
}
