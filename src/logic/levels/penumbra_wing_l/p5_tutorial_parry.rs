use std::f32::consts::TAU;

use crate::{
    Observed, i18n,
    logic::{
        TimeFinished, Timed,
        entities::{
            Killed, TryHurt,
            penumbra::{
                HomingTarget,
                bullet::{self, SpikyChargeEffect},
            },
        },
        levels::penumbra_wing_l::{Instance, Respawned, SeleneUi, p4_tutorial_launch},
    },
    math::RngExt as _,
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
        ring_radius,
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

    #[must_use = "commands aren't executed immediately"]
    fn try_fire(selene: Entity, e: impl Event + Copy) -> impl Command<Result> {
        move |world: &mut World| -> Result {
            let mut selene = world.get_entity_mut(selene)?;
            if selene.contains::<Disabled>() {
                selene.observe(move |trigger: Trigger<Respawned>, world: &mut World| {
                    world.despawn(trigger.observer());
                    world.trigger(e);
                });
            } else {
                world.trigger(e);
            }

            Ok(())
        }
    }

    // 1: Parry one bullet.
    commands.entity(level_entity).observe(
        move |trigger: Trigger<OnInsert, ParryOne>, mut commands: Commands, mut ui: ResMut<SeleneUi>| -> Result {
            #[derive(Debug, Copy, Clone, Event)]
            struct FireOne(u32);

            fn spawn_bullet(
                In(([level_entity, selene, attractor], ring_radius)): In<([Entity; 3], f32)>,
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

                let ignore_hit = commands
                    .spawn((
                        ChildOf(level_entity),
                        Observer::new(move |mut trigger: Trigger<TryHurt>| {
                            if trigger.by == bullet {
                                trigger.stop();
                            }
                        })
                        .with_entity(selene),
                    ))
                    .id();

                let pos = *attractor_pos + angle * ring_radius;
                commands
                    .spawn((ChildOf(level_entity), SpikyChargeEffect, Transform::from_translation(pos.extend(0.))))
                    .observe(move |trigger: Trigger<TimeFinished>, mut commands: Commands| {
                        commands.entity(trigger.target()).despawn();
                        commands
                            .entity(bullet)
                            .insert((
                                bullet::spiky(level_entity),
                                HomingTarget(selene),
                                LinearVelocity(angle * 96.),
                                Position(pos),
                                Rotation { cos: angle.x, sin: angle.y },
                            ))
                            .observe(move |_: Trigger<Killed>, mut commands: Commands| {
                                commands.entity(ignore_hit).despawn();
                            });
                    });

                Ok(bullet)
            }

            // This observer calls `spawn_bullet` after optionally displaying a dialog.
            commands.spawn((
                ChildOf(level_entity),
                Observer::new(move |trigger: Trigger<FireOne>, world: &mut World| -> Result {
                    let bullet = world.run_system_cached_with(spawn_bullet, ([level_entity, selene, attractor], ring_radius))??;

                    let num_fired = trigger.0;
                    world.entity_mut(bullet).observe(
                        move |trigger: Trigger<Killed>, mut commands: Commands, mut ui: ResMut<SeleneUi>| -> Result {
                            if trigger.by != selene {
                                match num_fired {
                                    1..=2 if trigger.by == trigger.target() => commands.queue(BottomDialog::show(
                                        None,
                                        i18n!(format!("tutorial.parry.fail.1-{num_fired}")),
                                        move |In(e): In<Entity>, mut commands: Commands| {
                                            commands.spawn((
                                                ChildOf(level_entity),
                                                Timed::run(Duration::from_secs(2), move |world: &mut World| -> Result {
                                                    BottomDialog::hide(e).apply(world)?;
                                                    try_fire(selene, FireOne(num_fired + 1)).apply(world)
                                                }),
                                            ));
                                        },
                                    )),
                                    num => {
                                        commands.spawn((
                                            ChildOf(level_entity),
                                            Timed::run(Duration::from_secs(1), move |world: &mut World| -> Result {
                                                try_fire(selene, FireOne(num)).apply(world)
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
                            try_fire(selene, FireOne(1)).apply(world)
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
                In(([level_entity, selene, attractor], ring_radius)): In<([Entity; 3], f32)>,
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

                let bullets = std::array::from_fn(|_| commands.spawn_empty().id());
                let incr = Rotation::radians(TAU / bullets.len() as f32);

                let ignore_hit = commands
                    .spawn((
                        ChildOf(level_entity),
                        Observer::new(move |mut trigger: Trigger<TryHurt>| {
                            if bullets.contains(&trigger.by) {
                                trigger.stop();
                            }
                        })
                        .with_entity(selene),
                    ))
                    .id();

                let mut remove_ignore_hit = Observer::new(move |_: Trigger<Killed>, mut commands: Commands, mut count: Local<usize>| {
                    *count += 1;
                    if *count == bullets.len() {
                        commands.entity(ignore_hit).despawn();
                    }
                });

                for (i, &bullet) in bullets.iter().enumerate() {
                    remove_ignore_hit.watch_entity(bullet);

                    let angle = base_angle;
                    let pos = *attractor_pos + vec2(angle.cos, angle.sin) * ring_radius;
                    base_angle *= incr;

                    let charge = (
                        ChildOf(level_entity),
                        SpikyChargeEffect,
                        Transform::from_translation(pos.extend(0.)),
                        Observed::by(move |trigger: Trigger<TimeFinished>, mut commands: Commands| {
                            commands.entity(trigger.target()).despawn();

                            // Bullet might've been despawned before it even appeared. Such is the case when Selene gets hit
                            // before all bullets are spawned. In that case, spawn a simple cancellation effect.
                            if let Ok(mut bullet) = commands.get_entity(bullet) {
                                // Use `try_insert` just in case.
                                bullet.try_insert((
                                    bullet::spiky(level_entity),
                                    HomingTarget(selene),
                                    LinearVelocity(vec2(angle.cos, angle.sin) * 96.),
                                    Position(pos),
                                    angle,
                                ));
                            } else {
                                // TODO "Pull away" effect immediately.
                            }
                        }),
                    );

                    if i == 0 {
                        commands.spawn(charge);
                    } else {
                        let mut charge = Some(charge);
                        commands.spawn((
                            ChildOf(level_entity),
                            Timed::run(Duration::from_millis(i as u64 * 250), move |mut commands: Commands| -> Result {
                                commands.spawn(charge.take().ok_or("`TimeFinished` fired twice")?);
                                Ok(())
                            }),
                        ));
                    }
                }

                commands.spawn((ChildOf(level_entity), remove_ignore_hit));
                Ok(bullets)
            }

            commands.spawn((
                ChildOf(level_entity),
                Observer::new(move |trigger: Trigger<FireMultiple>, world: &mut World| -> Result {
                    #[derive(Debug, Copy, Clone, Component)]
                    struct ParryCount(usize);

                    let bullets = world.run_system_cached_with(spawn_bullet, ([level_entity, selene, attractor], ring_radius))??;
                    let mut hit_observer = Observer::new(
                        move |trigger: Trigger<Killed>, mut commands: Commands, mut query: Query<&mut ParryCount>| -> Result {
                            if trigger.by == selene {
                                let mut count = query.get_mut(trigger.observer())?;
                                count.0 += 1;

                                if count.0 == bullets.len() {
                                    commands.entity(trigger.observer()).try_despawn();
                                }
                            } else {
                                for b in bullets {
                                    commands.entity(b).try_despawn();
                                }
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
                                                            BottomDialog::hide(e).apply(world)?;
                                                            try_fire(selene, FireMultiple(2)).apply(world)
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
                                                    BottomDialog::hide(e).apply(world)?;
                                                    try_fire(selene, FireMultiple(3)).apply(world)
                                                }),
                                            ));
                                        },
                                    )),
                                    num => {
                                        commands.spawn((
                                            ChildOf(level_entity),
                                            Timed::run(Duration::from_secs(1), move |world: &mut World| -> Result {
                                                try_fire(selene, FireMultiple(num)).apply(world)
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
            commands.queue(try_fire(selene, FireMultiple(1)));
        });

    Ok(())
}
