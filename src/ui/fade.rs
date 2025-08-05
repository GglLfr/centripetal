use std::time::Duration;

use bevy::{ecs::query::QueryItem, prelude::*};
use smallvec::SmallVec;

use crate::{
    Observed,
    logic::{TimeFinished, Timed},
};

#[derive(Debug, Clone, Default, Component)]
#[component(storage = "SparseSet")]
#[require(Timed::new(Duration::from_millis(750)))]
pub struct Fade {
    pub enter: bool,
    pub background: Color,
    pub box_shadow: SmallVec<[Color; 1]>,
    pub text: Color,
}

impl Fade {
    pub fn enter(e: Entity) -> impl Bundle + Clone {
        (
            ChildOf(e),
            Self {
                enter: true,
                ..default()
            },
            Observed::by(Timed::despawn_on_finished),
        )
    }

    pub fn exit(e: Entity) -> impl Bundle + Clone {
        (
            ChildOf(e),
            Self {
                enter: false,
                ..default()
            },
            Observed::by(Timed::despawn_on_finished),
        )
    }
}

pub type FadeSource<'a> = (&'a Fade, &'a Timed);

pub type FadeItem<'a> = (
    Option<&'a mut BackgroundColor>,
    Option<&'a mut BoxShadow>,
    Option<&'a mut TextColor>,
);

fn do_fade(
    (fade, timed): QueryItem<FadeSource>,
    (background_color, box_shadow, text_color): QueryItem<FadeItem>,
) {
    let f = if fade.enter {
        timed.frac()
    } else {
        1. - timed.frac()
    };

    if let Some(mut background_color) = background_color {
        background_color.0 = Color::NONE.mix(&fade.background, f);
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
    mut parent: Query<(&mut Fade, &ChildOf)>,
    query: Query<(
        Option<&BackgroundColor>,
        Option<&BoxShadow>,
        Option<&TextColor>,
    )>,
) {
    let Ok((mut fade, child_of)) = parent.get_mut(trigger.target()) else {
        return;
    };
    let Ok((background, box_shadow, text_color)) = query.get(child_of.parent()) else {
        return;
    };

    *fade = Fade {
        enter: fade.enter,
        background: background.map(|col| col.0).unwrap_or_default(),
        box_shadow: box_shadow
            .map(|box_shadow| box_shadow.iter().map(|sample| sample.color).collect())
            .unwrap_or_default(),
        text: text_color.map(|col| col.0).unwrap_or_default(),
    };
}

pub fn on_fade_done(
    trigger: Trigger<TimeFinished>,
    source: Query<(FadeSource, &ChildOf)>,
    mut item: Query<FadeItem>,
) {
    if let Ok((source, child_of)) = source.get(trigger.target())
        && let Ok(item) = item.get_mut(child_of.parent())
    {
        do_fade(source, item);
    }
}

pub fn fade_interpolate(source: Query<(FadeSource, &ChildOf)>, mut query: Query<FadeItem>) {
    for (source, child_of) in &source {
        if let Ok(item) = query.get_mut(child_of.parent()) {
            do_fade(source, item);
        }
    }
}
