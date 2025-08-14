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
};

use serde::{Deserialize, Serialize};

use crate::{
    SaveApp as _,
    logic::{
        Fields, FromLevel, LevelApp as _, LevelEntities,
        entities::penumbra::{AttractedInitial, Attractor},
        levels::in_level,
    },
    suspend,
};

pub mod p1_spawn_attractor;
pub mod p2_spawn_selene;
pub mod p3_tutorial_align;

const SELENE: Uuid = uuid!("332e5310-3740-11f0-b0d1-4b444b848a1e");
const ATTRACTOR: Uuid = uuid!("8226eab0-3740-11f0-b0d1-31c3cf318fb2");
const RINGS: [Uuid; 2] = [
    uuid!("483defc0-3740-11f0-bea9-1bca02df9366"),
    uuid!("516847d0-3740-11f0-bea9-db42cbfffb80"),
];
const HOVER_TARGET: Uuid = uuid!("ddc89020-3740-11f0-bea9-17dccf039850");

#[derive(
    Debug, Copy, Clone, Default, Resource, TypePath, Serialize, Deserialize, Deref, DerefMut,
)]
pub struct IntroShown(pub bool);

#[derive(Debug, Component)]
pub struct Instance {
    pub level_entity: Entity,
    pub selene: Entity,
    pub attractor: Entity,
    pub rings: [Entity; 2],
    pub hover_target: Entity,
    pub selene_initial: AttractedInitial,
    pub attractor_radius: f32,
    pub selene_trns: Transform,
    pub attractor_trns: Transform,
}

impl FromLevel for Instance {
    type Param = (
        SRes<IntroShown>,
        SQuery<Read<Transform>>,
        SQuery<Read<AttractedInitial>>,
        SQuery<Read<Attractor>>,
    );
    type Data = Read<LevelEntities>;

    fn from_level(
        mut e: EntityCommands,
        _: &Fields,
        (cutscene_shown, transforms, initials, attractors): SystemParamItem<Self::Param>,
        entities: QueryItem<Self::Data>,
    ) -> Result {
        if **cutscene_shown {
            // TODO Can this level be revisited without the cutscene?
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

        let selene_initial = initials.get(selene).copied().unwrap_or_default();
        let attractor_radius = attractors.get(attractor)?.radius;
        let [&selene_trns, &attractor_trns] = transforms.get_many([selene, attractor])?;

        commands.queue(move |world: &mut World| -> Result {
            let this = Self {
                level_entity,
                selene,
                attractor,
                rings: [ring_0, ring_1],
                hover_target,
                selene_initial,
                attractor_radius,
                selene_trns,
                attractor_trns,
            };

            world.run_system_cached_with(p1_spawn_attractor::init, &this)??;
            world.run_system_cached_with(p2_spawn_selene::init, &this)??;
            world.run_system_cached_with(p3_tutorial_align::init, &this)??;

            world.get_entity_mut(level_entity)?.insert(this);
            Ok(())
        });

        Ok(())
    }
}

/*
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
                                commands.entity(trigger.observer()).despawn();
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

        states = states
            .on_enter::<TutorialLaunch>(move |e| {
                e.commands().queue(move |world: &mut World| {
                    if world
                        .get_entity_mut(ui_selene_launch)
                        .map(ui_fade_in)
                        .is_ok()
                    {
                        world.resource_scope(
                            move |world: &mut World, mut shown_ui: Mut<ShownTutorialUi>| {
                                if let Ok(ui) = world.get_entity_mut(std::mem::replace(
                                    &mut **shown_ui,
                                    ui_selene_launch,
                                )) {
                                    ui_fade_out(ui);
                                }
                            },
                        );
                    }
                });

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
                            #[derive(Debug, Resource, Deref, DerefMut)]
                            struct HasHit(bool);

                            commands.insert_resource(HasHit(false));
                            for i in 0..3 {
                                let Rotation { cos, sin } = (*pos - *attractor_pos)
                                    .try_normalize()
                                    .map(|Vec2 { x: cos, y: sin }| Rotation { cos, sin })
                                    .unwrap_or_default()
                                    * Rotation::radians(TAU * i as f32 / 3.);

                                commands.spawn(Timed::run(
                                    Duration::from_millis((i as u64 + 1) * 150),
                                    move |_: In<Entity>, mut commands: Commands, hit: Res<HasHit>| {
                                        let mut bullet = commands.spawn((
                                            bullet::spiky(level_entity),
                                            LinearVelocity(vec2(cos * 156., sin * 156.)),
                                            attractor_pos,
                                            Rotation { cos, sin },
                                        ));

                                        if !**hit {
                                            bullet.insert(HomingTarget(selene)).observe(move |trigger: Trigger<OnCollisionStart>, mut commands: Commands| {
                                                if trigger.body.is_some_and(|body| body == selene) {
                                                    commands.trigger(Hit);
                                                    commands.insert_resource(HasHit(true));
                                                }
                                            });

                                            let id = bullet.id();
                                            bullet.commands().spawn((
                                                ChildOf(id),
                                                Observer::new(move |trigger: Trigger<Hit>, mut commands: Commands| {
                                                    if let Ok(mut e) = commands.get_entity(id) {
                                                        e.remove::<HomingTarget>();
                                                        commands.entity(trigger.observer()).despawn();
                                                    }
                                                }),
                                            ));
                                        }
                                    },
                                ));
                            }

                            let parried_or_dodged = commands
                                .spawn(Timed::run(
                                    Duration::from_secs(2),
                                    move |_: In<Entity>, mut commands: Commands| -> Result {
                                        commands.get_entity(level_entity)?.insert(Done::Success);
                                        Ok(())
                                    },
                                ))
                                .id();

                            commands.spawn((
                                ChildOf(level_entity),
                                Observer::new(move |trigger: Trigger<Hit>, mut commands: Commands| -> Result {
                                    commands.entity(trigger.observer()).despawn();
                                    if let Ok(mut e) = commands.get_entity(parried_or_dodged) {
                                        e.despawn();
                                        commands.get_entity(level_entity)?.insert(Done::Failure);
                                    }
                                    Ok(())
                                }),
                            ));

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
            .on_exit::<TutorialLaunch>(move |e| {
                e.commands().queue(move |world: &mut World| {
                    let temporary = world.spawn_empty().id();
                    world.resource_scope(
                        move |world: &mut World, mut shown_ui: Mut<ShownTutorialUi>| {
                            if let Ok(ui) =
                                world.get_entity_mut(std::mem::replace(&mut **shown_ui, temporary))
                            {
                                ui_fade_out(ui);
                            }
                        },
                    );
                });
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
}*/

pub(super) fn plugin(app: &mut App) {
    app.register_level::<Instance>("penumbra_wing_l")
        .add_systems(
            Update,
            (
                p2_spawn_selene::draw_spawn_effect, /*update_tutorial_move_aligned*/
            )
                .run_if(in_level("penumbra_wing_l")),
        )
        .save_resource_init::<IntroShown>();
}
