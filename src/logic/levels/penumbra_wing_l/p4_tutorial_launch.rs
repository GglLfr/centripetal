use std::f32::consts::TAU;

use crate::{
    i18n,
    logic::{
        TimeStun, Timed,
        entities::penumbra::{AttractedAction, HomingTarget, LaunchAction, Launched, bullet},
        levels::penumbra_wing_l::{Instance, SeleneUi, p3_tutorial_align},
    },
    prelude::*,
    resume,
    ui::{BottomDialog, WorldspaceUi, ui_fade_in, ui_fade_out, ui_hide, widgets},
};

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
pub enum TutorialLaunch {
    Normal,
    #[default]
    Special,
}

pub fn init(
    InRef(&Instance {
        level_entity,
        selene,
        attractor,
        rings,
        ..
    }): InRef<Instance>,
    mut commands: Commands,
) -> Result {
    let ui_selene_launch = commands
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
                            widgets::keyboard_binding(|binds| binds.launch),
                            TextColor(Color::BLACK),
                        )],
                    )],
                ),
                (
                    Node::default(),
                    children![(
                        widgets::shadow_bg(),
                        widgets::text(i18n!("tutorial.hover.launch")),
                        TextLayout::new(JustifyText::Left, LineBreak::NoWrap),
                    )],
                ),
            ],
        ))
        .queue(ui_hide)
        .id();

    // Entry point.
    commands.entity(level_entity).observe(
        move |trigger: Trigger<OnRemove, p3_tutorial_align::TutorialAlign>,
              mut ui: ResMut<SeleneUi>,
              mut commands: Commands,
              mut actions: Query<&mut ActionState<LaunchAction>>|
              -> Result {
            // Default to `Special` state; see below on bullet hits.
            commands.entity(trigger.observer()).despawn();
            commands
                .entity(level_entity)
                .insert(TutorialLaunch::Special);

            // `Launch` is enabled now.
            actions.get_mut(selene)?.enable_action(&LaunchAction);

            // Replace the Selene UI with a prompt to launch.
            commands.get_entity(ui_selene_launch)?.queue(ui_fade_in);
            if let Some(ui) = ui.replace(ui_selene_launch)
                && let Ok(mut ui) = commands.get_entity(ui)
            {
                ui.queue(ui_fade_out);
            }

            // On launch, do some very specific things:
            commands.get_entity(selene)?.observe(
                move |trigger: Trigger<Launched>,
                      mut ui: ResMut<SeleneUi>,
                      mut commands: Commands,
                      positions: Query<&Position>,
                      mut actions: Query<&mut ActionState<AttractedAction>>|
                      -> Result {
                    if trigger.at == attractor {
                        let [&pos, &attractor_pos] =
                            positions.get_many([trigger.target(), trigger.at])?;

                        // 1: Secretly enable parrying.
                        actions
                            .get_mut(trigger.target())?
                            .enable_action(&AttractedAction::Parry);

                        // 2: Queue a time stun.
                        commands.entity(trigger.observer()).despawn();
                        commands.spawn((ChildOf(level_entity), TimeStun::long_smooth()));

                        // 3: Spawn 3 bullets that, when at least one hits Selene, will trigger a "normal" branch.
                        //    Otherwise, trigger a "special" branch for phase 5.
                        let bullets = [0, 1, 2].map(|i| {
                            let Rotation { cos, sin } = (*pos - *attractor_pos)
                                .try_normalize()
                                .map(|Vec2 { x: cos, y: sin }| Rotation { cos, sin })
                                .unwrap_or_default()
                                * Rotation::radians(TAU * i as f32 / 3.);

                            let bullet = commands.spawn_empty().id();
                            commands.spawn(Timed::run(
                                Duration::from_millis((i as u64 + 1) * 150),
                                move |_: In<Entity>,
                                      mut commands: Commands,
                                      query: Query<&TutorialLaunch>|{
                                    let mut bullet = commands.entity(bullet);
                                    bullet
                                        .insert((
                                            bullet::spiky(level_entity),
                                            LinearVelocity(vec2(cos * 156., sin * 156.)),
                                            attractor_pos,
                                            Rotation { cos, sin },
                                        ));

                                    // Don't home in if one of the bullets already hit.
                                    if query.get(level_entity).is_ok_and(|tutorial| matches!(tutorial, TutorialLaunch::Special)) {
                                        bullet.insert(HomingTarget(selene));
                                    }
                                },
                            ));

                            bullet
                        });

                        let mut hit_observer = Observer::new(
                            move |trigger: Trigger<OnCollisionStart>,
                                  mut commands: Commands,
                                  mut query: Query<&mut TutorialLaunch>|
                                  -> Result {
                                commands.entity(trigger.observer()).despawn();
                                *query.get_mut(level_entity)? = TutorialLaunch::Normal;

                                for bullet in bullets {
                                    if let Ok(mut bullet) = commands.get_entity(bullet) {
                                        bullet.remove::<HomingTarget>();
                                    }
                                }

                                Ok(())
                            },
                        );

                        let mut count = bullets.len();
                        let mut despawn_observer = Observer::new(
                            move |trigger: Trigger<OnRemove, RigidBody>,
                                  mut commands: Commands|
                                  -> Result {
                                if let Some(new_count) = count.checked_sub(1) {
                                    count = new_count;
                                } else {
                                    commands.entity(trigger.observer()).despawn();
                                    commands
                                        .get_entity(level_entity)?
                                        .remove::<TutorialLaunch>();
                                }
                                Ok(())
                            },
                        );

                        for bullet in bullets {
                            hit_observer.watch_entity(bullet);
                            despawn_observer.watch_entity(bullet);
                        }

                        commands.spawn((ChildOf(level_entity), hit_observer));
                        commands.spawn((ChildOf(level_entity), despawn_observer));

                        // 4: Spawn 2 rings that protect the attractor from being slashed by Selene.
                        commands.spawn(Timed::run(
                            Duration::from_millis(750),
                            move |_: In<Entity>, mut commands: Commands| -> Result {
                                for ring in rings {
                                    // TODO FX for this.
                                    commands.get_entity(ring)?.queue(resume);
                                }
                                Ok(())
                            },
                        ));

                        // 5: Hide the launching UI.
                        if let Some(ui) = ui.take()
                            && ui == ui_selene_launch
                            && let Ok(mut ui) = commands.get_entity(ui)
                        {
                            ui.queue(ui_fade_out);
                        }

                        // 6: Spawn a "Ahh! Away from me!" dialog residing at the bottom.
                        commands.queue(BottomDialog::show(i18n!("tutorial.launch.scream"), BottomDialog::hide_after(Duration::from_secs(3))));

                        /*commands.spawn(Timed::run(Duration::from_millis(100), move |_: In<Entity>, mut commands: Commands| {
                            commands.spawn((
                                Node {
                                    width: Percent(100.),
                                    height: Percent(100.),
                                    flex_direction: FlexDirection::Column,
                                    ..default()
                                },
                                children![(
                                    Node {
                                        display: Display::Grid,
                                        margin: UiRect::new(Px(0.), Px(0.), Auto, Px(0.)),
                                        align_self: AlignSelf::Center,
                                        ..default()
                                    },
                                    children![(
                                        Node {
                                            grid_row: GridPlacement::start(1),
                                            grid_column: GridPlacement::start(1),
                                            ..default()
                                        },
                                        widgets::scroll_text(i18n!("tutorial.launch.scream")),
                                    ), (
                                        Node {
                                            grid_row: GridPlacement::start(1),
                                            grid_column: GridPlacement::start(1),
                                            ..default()
                                        },
                                        widgets::text(i18n!("tutorial.launch.scream")),
                                        Visibility::Hidden,
                                    )],
                                )],
                            ));
                        }));*/
                    }

                    Ok(())
                },
            );

            Ok(())
        },
    );

    Ok(())
}
