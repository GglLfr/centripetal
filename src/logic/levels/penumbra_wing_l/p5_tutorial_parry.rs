use std::f32::consts::TAU;

use crate::{
    despawn, i18n,
    logic::{
        Timed,
        entities::{
            ParryCollider, TryHurt,
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

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
struct ParryDone;

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
            commands.queue(despawn(trigger.observer()));
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
                            Timed::run(Duration::from_secs(2), move |mut commands: Commands| -> Result {
                                // If the player already knows how to parry, skip it altogether.
                                // This is intended as a speedrun mechanic.
                                if skip {
                                    commands.get_entity(level_entity)?.insert(ParryDone);
                                } else {
                                    commands.get_entity(level_entity)?.insert(ParryOne);
                                }
                                Ok(())
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
            struct Fire(u32);
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
                    ChildOf(selene),
                    Observer::new(move |mut trigger: Trigger<TryHurt>, mut commands: Commands| {
                        commands.queue(despawn(trigger.observer()));
                        if trigger.by == bullet {
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
                        LinearVelocity(angle * 128.),
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
                Observer::new(move |trigger: Trigger<Fire>, world: &mut World| -> Result {
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
                                    1..2 => commands.queue(BottomDialog::show(
                                        None,
                                        i18n!(format!("tutorial.parry.fail.1-{num_fired}")),
                                        move |In(e): In<Entity>, mut commands: Commands| {
                                            commands.spawn((
                                                ChildOf(level_entity),
                                                Timed::run(Duration::from_secs(2), move |world: &mut World| -> Result {
                                                    world.trigger_targets(Fire(num_fired + 1), level_entity);
                                                    BottomDialog::hide(e).apply(world)
                                                }),
                                            ));
                                        },
                                    )),
                                    _ => {
                                        commands.spawn((
                                            ChildOf(level_entity),
                                            Timed::run(Duration::from_secs(1), move |world: &mut World| {
                                                world.trigger_targets(Fire(3), level_entity);
                                            }),
                                        ));
                                    }
                                }
                            } else {
                                commands.queue(despawn(trigger.observer()));
                                commands.get_entity(level_entity)?.remove::<ParryOne>().insert(ParryMultiple);

                                if let Some(ui) = ui.take_if(|&mut ui| ui == ui_selene_parry) {
                                    commands.entity(ui).queue(ui_fade_out);
                                }
                            }

                            Ok(())
                        },
                    );

                    Ok(())
                })
                .with_entity(level_entity),
            ));

            // Show some initial dialog...
            commands.queue(despawn(trigger.observer()));
            commands.queue(BottomDialog::show(
                None,
                i18n!("tutorial.parry.aid"),
                move |In(e): In<Entity>, mut commands: Commands| {
                    commands.spawn((
                        ChildOf(level_entity),
                        Timed::run(Duration::from_secs(2), move |world: &mut World| -> Result {
                            // ...then start firing the bullet.
                            BottomDialog::hide(e).apply(world)?;
                            world.trigger_targets(Fire(1), level_entity);

                            Ok(())
                        }),
                    ));
                },
            ));

            commands.get_entity(ui_selene_parry)?.queue(ui_fade_in);
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
            commands.queue(despawn(trigger.observer()));
            commands.queue(BottomDialog::show(
                None,
                i18n!("tutorial.parry.success.1.good"),
                BottomDialog::show_next_after(
                    Duration::from_secs(2),
                    i18n!("tutorial.parry.success.1.proceed"),
                    move |In(e): In<Entity>, mut commands: Commands| {
                        commands.spawn((
                            ChildOf(level_entity),
                            Timed::run(Duration::from_secs(2), move |world: &mut World| -> Result {
                                BottomDialog::hide(e).apply(world)?;
                                Ok(())
                            }),
                        ));
                    },
                ),
            ));
        });

    Ok(())
}
