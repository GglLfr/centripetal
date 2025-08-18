use std::f32::consts::TAU;

use crate::{
    Sprites,
    graphics::{Animation, AnimationMode, BaseColor},
    i18n,
    logic::{
        Timed,
        effects::Ring,
        entities::{
            EntityLayers, Killed,
            penumbra::{AttractedAction, LaunchAction},
        },
        levels::penumbra_wing_l::{
            Instance, SeleneUi,
            p2_spawn_selene::{self, Respawned},
        },
    },
    prelude::*,
    resume,
    ui::{WorldspaceUi, ui_fade_in, ui_fade_out, ui_hide, widgets},
};

const TUTORIAL_MOVE_ALIGN_HELP: Duration = Duration::from_millis(500);
const TUTORIAL_MOVE_ALIGN_DURATION: Duration = Duration::from_secs(5);

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
pub struct TutorialAlign {
    time: Duration,
    within: bool,
}

pub fn update_align_time(
    mut commands: Commands,
    time: Res<Time>,
    mut aligned: Query<(&Instance, &mut TutorialAlign)>,
    mut target: Query<(&mut ShapeMaterial, &mut DiscComponent)>,
) {
    let delta: Duration = time.delta();
    let Ok((instance, mut align)) = aligned.single_mut() else { return };

    align.time = if align.within { (align.time + delta).min(TUTORIAL_MOVE_ALIGN_DURATION) } else { align.time.saturating_sub(delta) };

    if align.time == TUTORIAL_MOVE_ALIGN_DURATION {
        // TODO FX for this.
        commands.entity(instance.hover_target).despawn();
        commands.entity(instance.level_entity).remove::<TutorialAlign>();
    }

    let Ok((mut material, mut disc)) = target.get_mut(instance.hover_target) else { return };
    let (disable_laa, cap) = if align.time > Duration::ZERO { (false, Cap::Round) } else { (true, Cap::None) };

    disc.end_angle = TAU * align.time.div_duration_f32(TUTORIAL_MOVE_ALIGN_DURATION);
    disc.cap = cap;
    material.disable_laa = disable_laa;
}

pub fn init(
    InRef(&Instance {
        level_entity,
        selene,
        hover_target,
        ..
    }): InRef<Instance>,
    mut commands: Commands,
    shapes: ShapeCommands,
    sprites: Res<Sprites>,
) -> Result {
    commands.entity(hover_target).insert((
        EntityLayers::penumbra_hostile(),
        Collider::circle(8.),
        CollisionEventsEnabled,
        Sensor,
        Animation::new(sprites.collectible_32.clone_weak(), "anim"),
        AnimationMode::Repeat,
        BaseColor(Color::linear_rgb(12., 2., 1.)),
        DiscComponent::arc(shapes.config(), 16., 0., 0.),
        ShapeMaterial {
            alpha_mode: ShapeAlphaMode::Blend,
            disable_laa: true,
            pipeline: ShapePipelineType::Shape2d,
            canvas: None,
            texture: None,
        },
        ShapeFill {
            color: Color::linear_rgb(4., 2., 1.),
            ty: FillType::Stroke(1., ThicknessType::World),
        },
        Timed::repeat(Duration::from_secs(1), |In(e): In<Entity>, mut commands: Commands| {
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
        }),
        DebugRender::none(),
    ));

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
                    children![(widgets::icon(), children![(
                        widgets::keyboard_binding(|binds| binds.attracted_hover[0]),
                        TextColor(Color::BLACK),
                    )],)],
                ),
                (Node::default(), children![(
                    widgets::shadow_bg(),
                    widgets::text(i18n!("tutorial.hover.descend")),
                    TextLayout::new(JustifyText::Left, LineBreak::NoWrap),
                )],),
                (
                    Node {
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::End,
                        ..default()
                    },
                    children![(widgets::icon(), children![(
                        widgets::keyboard_binding(|binds| binds.attracted_hover[1]),
                        TextColor(Color::BLACK),
                    )],)],
                ),
                (Node::default(), children![(
                    widgets::shadow_bg(),
                    widgets::text(i18n!("tutorial.hover.ascend")),
                    TextLayout::new(JustifyText::Left, LineBreak::NoWrap),
                )],),
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
                    children![(widgets::icon(), children![(
                        widgets::keyboard_binding(|binds| binds.attracted_accel[0]),
                        TextColor(Color::BLACK),
                    )],)],
                ),
                (Node::default(), children![(
                    widgets::shadow_bg(),
                    widgets::text(i18n!("tutorial.hover.retrograde")),
                    TextLayout::new(JustifyText::Left, LineBreak::NoWrap),
                )],),
                (
                    Node {
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::End,
                        ..default()
                    },
                    children![(widgets::icon(), children![(
                        widgets::keyboard_binding(|binds| binds.attracted_accel[1]),
                        TextColor(Color::BLACK),
                    )],)],
                ),
                (Node::default(), children![(
                    widgets::shadow_bg(),
                    widgets::text(i18n!("tutorial.hover.prograde")),
                    TextLayout::new(JustifyText::Left, LineBreak::NoWrap),
                )],),
            ],
        ))
        .queue(ui_hide)
        .id();

    // Fade out UI during death and fade it back in on respawn.
    commands
        .entity(selene)
        .observe(move |_: Trigger<Killed>, mut commands: Commands, mut ui: ResMut<SeleneUi>| {
            if let Some(e) = ui.take()
                && let Ok(mut e) = commands.get_entity(e)
            {
                e.queue(ui_fade_out);

                let ui_entity = e.id();
                commands
                    .entity(selene)
                    .observe(move |trigger: Trigger<Respawned>, mut commands: Commands, mut ui: ResMut<SeleneUi>| {
                        commands.entity(trigger.observer()).despawn();
                        // Needs `is_none()` here to ensure we don't accidentally replace an existing UI.
                        // Such condition should be considered a bug; this is only a failsafe.
                        if ui.is_none()
                            && let Ok(mut e) = commands.get_entity(ui_entity)
                        {
                            e.queue(ui_fade_in);
                            **ui = Some(e.id());
                        }
                    });
            }
        });

    // Entry point.
    // Refer to `update_align_time` for when this phase is finished.
    commands.entity(level_entity).observe(
        move |trigger: Trigger<OnRemove, p2_spawn_selene::SpawningSelene>,
              mut commands: Commands,
              mut ui: ResMut<SeleneUi>,
              mut actions: Query<(&mut ActionState<AttractedAction>, &mut ActionState<LaunchAction>)>|
              -> Result {
            commands.entity(trigger.observer()).despawn();
            commands.entity(level_entity).insert(TutorialAlign::default());

            **ui = Some(ui_selene_hover);
            commands.get_entity(ui_selene_hover)?.queue(ui_fade_in);

            // Only `Hover` and `Accel` are enabled initially.
            let (mut attracted, mut launch) = actions.get_mut(selene)?;
            attracted.disable_action(&AttractedAction::Parry);
            launch.disable_action(&LaunchAction);

            commands
                .get_entity(hover_target)?
                .queue(resume)
                // Set `within = true` when Selene overlaps to increment the counter.
                .observe(
                    move |trigger: Trigger<OnCollisionStart>, mut aligned: Query<&mut TutorialAlign>| -> Result {
                        if trigger.body.is_some_and(|body| body == selene) {
                            aligned.get_mut(level_entity)?.within = true;
                        }

                        Ok(())
                    },
                )
                // Otherwise, set `within = false` to decrement the counter.
                .observe(
                    move |trigger: Trigger<OnCollisionEnd>,
                          mut commands: Commands,
                          mut shown_ui: ResMut<SeleneUi>,
                          mut aligned: Query<&mut TutorialAlign>,
                          mut hinted: Local<bool>|
                          -> Result {
                        // Hint about prograding/retrograding when unaligning.
                        let mut aligned = aligned.get_mut(level_entity)?;
                        if trigger.body.is_some_and(|body| body == selene)
                            && std::mem::replace(&mut aligned.within, false)
                            && aligned.time >= TUTORIAL_MOVE_ALIGN_HELP
                            && !std::mem::replace(&mut *hinted, true)
                            && commands
                                .get_entity(ui_selene_accel)
                                .map(|mut e| {
                                    e.queue(ui_fade_in);
                                })
                                .is_ok()
                            && let Some(ui) = shown_ui.replace(ui_selene_accel)
                            && let Ok(mut ui) = commands.get_entity(ui)
                        {
                            ui.queue(ui_fade_out);
                        }

                        Ok(())
                    },
                );

            Ok(())
        },
    );

    Ok(())
}
