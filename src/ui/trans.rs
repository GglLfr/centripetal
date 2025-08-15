use crate::{
    Observed, WithChild,
    logic::Timed,
    math::FloatTransformExt as _,
    prelude::*,
    ui::{ui_hide, ui_show},
};

pub const UI_STRETCH_TIME: Duration = Duration::from_millis(100);
pub const UI_FADE_TIME: Duration = Duration::from_millis(750);

#[derive(Debug, Clone, Default, Component)]
#[require(Timed::new(UI_STRETCH_TIME))]
pub struct Stretch {
    pub enter: bool,
    pub target: Vec3,
    pub vertical: bool,
}

impl Stretch {
    pub fn bundle(enter: bool, vertical: bool) -> impl Bundle + Clone {
        WithChild((
            Self {
                enter,
                vertical,
                ..default()
            },
            Observed::by(Timed::despawn_on_finished),
        ))
    }

    pub fn enter_vertical() -> impl Bundle + Clone {
        Self::bundle(true, true)
    }

    pub fn exit_vertical() -> impl Bundle + Clone {
        Self::bundle(false, true)
    }

    pub fn enter_horizontal() -> impl Bundle + Clone {
        Self::bundle(true, false)
    }

    pub fn exit_horizontal() -> impl Bundle + Clone {
        Self::bundle(false, false)
    }
}

pub fn on_stretch_insert(
    trigger: Trigger<OnInsert, Stretch>,
    mut commands: Commands,
    mut parent: Query<(&mut Stretch, &ChildOf)>,
    mut query: Query<&mut Transform>,
) {
    let Ok((mut stretch, child_of)) = parent.get_mut(trigger.target()) else {
        return;
    };
    let Ok(mut trns) = query.get_mut(child_of.parent()) else {
        return;
    };

    let scale = trns.scale;
    stretch.target = if stretch.enter {
        std::mem::replace(
            &mut trns.scale,
            vec3(
                if stretch.vertical { scale.x } else { 0. },
                if stretch.vertical { 0. } else { scale.y },
                scale.z,
            ),
        )
    } else {
        scale
    };

    commands.entity(child_of.parent()).queue(ui_show);
}

pub fn on_stretch_replace(
    trigger: Trigger<OnReplace, Stretch>,
    mut commands: Commands,
    parent: Query<(&Stretch, &ChildOf)>,
    mut query: Query<&mut Transform>,
) {
    if let Ok((stretch, child_of)) = parent.get(trigger.target())
        && let Ok(mut trns) = query.get_mut(child_of.parent())
    {
        trns.scale = stretch.target;
        if !stretch.enter {
            commands.entity(child_of.parent()).queue(ui_hide);
        }
    }
}

pub fn stretch_interpolate(
    parent: Query<(&Stretch, &Timed, &ChildOf)>,
    mut query: Query<&mut Transform>,
) {
    for (stretch, timed, child_of) in &parent {
        if let Ok(mut trns) = query.get_mut(child_of.parent()) {
            let t = stretch.target;
            trns.scale = vec3(
                if stretch.vertical { t.x } else { 0. },
                if stretch.vertical { 0. } else { t.y },
                t.z,
            )
            .lerp(
                t,
                if stretch.enter {
                    timed.frac().pow_in(3)
                } else {
                    1. - timed.frac().pow_in(3)
                },
            );
        }
    }
}

#[derive(Debug, Clone, Default, Component)]
#[require(Timed::new(UI_FADE_TIME))]
pub struct Fade {
    pub enter: bool,
    pub background: Color,
    pub border: Color,
    pub box_shadow: SmallVec<[Color; 1]>,
    pub text: Color,
}

impl Fade {
    pub fn enter() -> impl Bundle + Clone {
        WithChild((
            Self {
                enter: true,
                ..default()
            },
            Observed::by(Timed::despawn_on_finished),
        ))
    }

    pub fn exit() -> impl Bundle + Clone {
        WithChild((
            Self {
                enter: false,
                ..default()
            },
            Observed::by(Timed::despawn_on_finished),
        ))
    }
}

pub type FadeSource<'a> = (&'a Fade, &'a Timed);

pub type FadeItem<'a> = (
    Option<&'a mut BackgroundColor>,
    Option<&'a mut BorderColor>,
    Option<&'a mut BoxShadow>,
    Option<&'a mut TextColor>,
);

fn do_fade(
    (fade, timed): QueryItem<FadeSource>,
    (background_color, border_color, box_shadow, text_color): QueryItem<FadeItem>,
) {
    let f = if fade.enter {
        timed.frac()
    } else {
        1. - timed.frac()
    };

    if let Some(mut background_color) = background_color {
        background_color.0 = Color::NONE.mix(&fade.background, f);
    }

    if let Some(mut border_color) = border_color {
        border_color.0 = Color::NONE.mix(&fade.border, f);
    }

    if let Some(mut box_shadow) = box_shadow {
        for (dst, src) in box_shadow
            .iter_mut()
            .map(|sample| &mut sample.color)
            .zip(&fade.box_shadow)
        {
            *dst = Color::NONE.mix(src, f);
        }
    }

    if let Some(mut text_color) = text_color {
        text_color.0 = Color::NONE.mix(&fade.text, f);
    }
}

pub fn on_fade_insert(
    trigger: Trigger<OnInsert, Fade>,
    mut commands: Commands,
    mut parent: Query<(&mut Fade, &ChildOf)>,
    mut query: Query<(Entity, FadeItem)>,
) {
    let Ok((mut fade, child_of)) = parent.get_mut(trigger.target()) else {
        return;
    };
    let Ok((e, (background, border, box_shadow, text_color))) = query.get_mut(child_of.parent())
    else {
        return;
    };

    *fade = if fade.enter {
        Fade {
            enter: true,
            background: background
                .map(|mut col| std::mem::replace(&mut col.0, Color::NONE))
                .unwrap_or_default(),
            border: border
                .map(|mut col| std::mem::replace(&mut col.0, Color::NONE))
                .unwrap_or_default(),
            box_shadow: box_shadow
                .map(|mut box_shadow| {
                    box_shadow
                        .iter_mut()
                        .map(|sample| std::mem::replace(&mut sample.color, Color::NONE))
                        .collect()
                })
                .unwrap_or_default(),
            text: text_color
                .map(|mut col| std::mem::replace(&mut col.0, Color::NONE))
                .unwrap_or_default(),
        }
    } else {
        Fade {
            enter: false,
            background: background.map(|col| col.0).unwrap_or_default(),
            border: border.map(|col| col.0).unwrap_or_default(),
            box_shadow: box_shadow
                .map(|box_shadow| box_shadow.iter().map(|sample| sample.color).collect())
                .unwrap_or_default(),
            text: text_color.map(|col| col.0).unwrap_or_default(),
        }
    };

    commands.entity(e).queue(ui_show);
}

pub fn on_fade_replace(
    trigger: Trigger<OnReplace, Fade>,
    mut commands: Commands,
    source: Query<(FadeSource, &ChildOf)>,
    mut item: Query<(Entity, FadeItem)>,
) {
    if let Ok(((fade, timed), child_of)) = source.get(trigger.target())
        && let Ok((e, item)) = item.get_mut(child_of.parent())
    {
        do_fade(
            (
                &Fade {
                    // `enter: true` to make it return to its original colors.
                    // `timed.frac()` returns `1.` here; see below.
                    enter: true,
                    ..fade.clone()
                },
                // Make it as if the timer has finished already.
                &Timed::new_at(timed.lifetime, timed.lifetime),
            ),
            item,
        );

        if !fade.enter {
            commands.entity(e).queue(ui_hide);
        }
    }
}

pub fn fade_interpolate(source: Query<(FadeSource, &ChildOf)>, mut query: Query<FadeItem>) {
    for (source, child_of) in &source {
        if let Ok(item) = query.get_mut(child_of.parent()) {
            do_fade(source, item);
        }
    }
}
