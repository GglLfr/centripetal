use std::time::Duration;

use bevy::{
    prelude::*,
    text::{FontSmoothing, LineHeight},
    ui::{UiSystem, Val::*},
};

mod scroll_text;
pub use scroll_text::*;

use crate::{Fonts, I18n, Observed, OnI18nNotify};

pub fn shadow_bg() -> impl Bundle {
    BoxShadow::new(
        Color::linear_rgba(0., 0., 0., 0.85),
        Px(0.),
        Px(0.),
        Px(-5.),
        Px(5.),
    )
}

pub fn scroll_text(i18n: I18n) -> impl Bundle {
    (
        i18n,
        Text::default(),
        ScrollText::default(),
        Observed::by(
            |trigger: Trigger<OnI18nNotify>,
             fonts: Res<Fonts>,
             mut query: Query<(&mut ScrollText, &mut ScrollTextState)>|
             -> Result {
                let (mut text, mut state) = query.get_mut(trigger.target())?;
                text.sections.clear();

                trigger.format(
                    |key| trigger.arguments.get(key).map(String::as_str),
                    |string, style| {
                        text.sections.push(ScrollTextSection {
                            span: string.into(),
                            font: TextFont {
                                font: match (style.bold, style.italic) {
                                    (true, true) => fonts.bold_italic.clone_weak(),
                                    (true, false) => fonts.bold.clone_weak(),
                                    (false, true) => fonts.italic.clone_weak(),
                                    (false, false) => fonts.regular.clone_weak(),
                                },
                                font_size: style.size as f32,
                                line_height: LineHeight::RelativeToFont(1.5),
                                font_smoothing: FontSmoothing::AntiAliased,
                            },
                            color: style.color.into(),
                            time_per_char: Duration::from_millis(16),
                        });
                    },
                );

                *state = default();
                Ok(())
            },
        ),
    )
}

#[derive(Debug, Copy, Clone, Default)]
pub struct WidgetPlugin;
impl Plugin for WidgetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            update_scroll_text_sections.before(UiSystem::Layout),
        );
    }
}
