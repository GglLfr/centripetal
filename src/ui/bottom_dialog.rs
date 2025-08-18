use crate::{
    I18n, IntoResultSystem, Observed,
    logic::{TimeFinished, Timed},
    prelude::*,
    ui::{
        UI_STRETCH_TIME, ui_stretch_horizontal_in, ui_stretch_horizontal_out,
        widgets::{self, ScrollTextFinished},
    },
};

#[derive(Debug, Clone, Resource)]
pub struct BottomDialog {
    root: Entity,
    current: Option<Entity>,
}

impl FromWorld for BottomDialog {
    fn from_world(world: &mut World) -> Self {
        let full = world
            .spawn(Node {
                width: Percent(100.),
                height: Percent(100.),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .id();

        Self {
            root: world
                .spawn((ChildOf(full), Node {
                    min_width: Px(640.),
                    width: Percent(80.),
                    height: Percent(100.),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::bottom(Px(4.)),
                    ..default()
                }))
                .id(),
            current: None,
        }
    }
}

impl BottomDialog {
    #[must_use = "this returns a `Command` that must be queued"]
    pub fn show<M>(previous: impl Into<Option<Entity>>, i18n: I18n, on_done: impl IntoResultSystem<In<Entity>, (), M>) -> impl Command<Result> {
        let previous = previous.into();
        let mut on_done = Some(IntoResultSystem::into_system(on_done));

        move |world: &mut World| {
            let ui = world.spawn_empty().id();
            let mut this = world.get_resource_mut::<Self>().ok_or("`BottomDialog` missing")?;

            if this.current.zip(previous).is_some_and(|(current, previous)| current != previous) {
                return Ok(())
            }

            let root = this.root;
            let prev = this.current.replace(ui);
            world.entity_mut(ui).insert((
                ChildOf(root),
                Node {
                    display: Display::Grid,
                    margin: UiRect::new(Px(0.), Px(0.), Auto, Px(0.)),
                    padding: UiRect::axes(Px(6.), Px(4.)),
                    align_self: AlignSelf::Center,
                    border: UiRect::all(Px(2.)),
                    ..default()
                },
                BorderColor(Srgba::hex("#ECF8FB").unwrap().into()),
                BackgroundColor(Color::linear_rgba(0., 0., 0., 0.4)),
                BoxShadow::new(Color::linear_rgba(0., 0., 0., 0.4), Px(0.), Px(0.), Px(5.), Px(5.)),
                BorderRadius::all(Px(12.)),
                children![
                    (
                        Node {
                            grid_row: GridPlacement::start(1),
                            grid_column: GridPlacement::start(1),
                            ..default()
                        },
                        widgets::scroll_text(i18n.clone()),
                        Observed::by(move |trigger: Trigger<ScrollTextFinished>, mut commands: Commands| -> Result {
                            commands.entity(trigger.observer()).despawn();
                            let mut on_done = on_done.take().ok_or("`ScrollTextFinished` fired twice")?;

                            commands.queue(move |world: &mut World| -> Result {
                                on_done.initialize(world);
                                on_done
                                    .validate_param(world)
                                    .map_err(|err| RunSystemError::InvalidParams { system: on_done.name(), err })?;
                                on_done.run(ui, world)
                            });

                            Ok(())
                        }),
                    ),
                    (
                        Node {
                            grid_row: GridPlacement::start(1),
                            grid_column: GridPlacement::start(1),
                            ..default()
                        },
                        widgets::text(i18n),
                        Visibility::Hidden,
                    )
                ],
            ));

            if let Some(prev) = prev {
                world.despawn(prev);
            } else {
                ui_stretch_horizontal_in(world.entity_mut(ui));
            }

            Ok(())
        }
    }

    pub fn hide(previous: impl Into<Option<Entity>>) -> impl Command<Result> {
        let previous = previous.into();
        move |world: &mut World| {
            let mut this = world.get_resource_mut::<Self>().ok_or("`BottomDialog` missing")?;
            if this.current.zip(previous).is_some_and(|(current, previous)| current != previous) {
                return Ok(())
            }

            if let Some(prev) = this.current.take()
                && world.entities().contains(prev)
            {
                ui_stretch_horizontal_out(world.entity_mut(prev));
                world.spawn((
                    ChildOf(prev),
                    Timed::run(UI_STRETCH_TIME, move |mut commands: Commands| {
                        commands.entity(prev).try_despawn();
                    }),
                ));
            }

            Ok(())
        }
    }

    #[must_use = "this returns a finish listener that must be used in conjunction with `show()`"]
    pub fn show_next_after<M>(
        duration: Duration,
        i18n: I18n,
        on_done: impl IntoResultSystem<In<Entity>, (), M>,
    ) -> impl System<In = In<Entity>, Out = Result> {
        let mut on_done = Some(IntoResultSystem::into_system(on_done));
        IntoSystem::into_system(move |In(e): In<Entity>, mut commands: Commands| {
            let mut on_done = on_done.take();
            let i18n = i18n.clone();

            commands
                .entity(e)
                .insert(Timed::new(duration))
                .observe(move |trigger: Trigger<TimeFinished>, world: &mut World| -> Result {
                    let on_done = on_done.take().ok_or("`TimeFinished` fired twice")?;
                    Self::show(trigger.target(), i18n.clone(), on_done).apply(world)?;

                    Ok(())
                });
            Ok(())
        })
    }

    pub fn hide_after(duration: Duration) -> impl System<In = In<Entity>, Out = Result> {
        IntoSystem::into_system(move |In(e): In<Entity>, mut commands: Commands| {
            commands
                .entity(e)
                .insert(Timed::new(duration))
                .observe(move |_: Trigger<TimeFinished>, world: &mut World| -> Result { Self::hide(e).apply(world) });
            Ok(())
        })
    }
}
