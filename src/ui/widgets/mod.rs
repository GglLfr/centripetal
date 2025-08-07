use std::time::Duration;

use bevy::{
    prelude::*,
    text::{FontSmoothing, LineHeight, TextBounds},
    ui::{UiSystem, Val::*},
};

mod scroll_text;
pub use scroll_text::*;

use crate::{
    Affected, Config, Fonts, I18n, I18nNotify, I18nNotifyCommand, KeyboardBindings, Observed,
    Rebind, keycode_desc,
};

pub fn shadow_bg() -> impl Bundle {
    BoxShadow::new(
        Color::linear_rgba(0., 0., 0., 0.85),
        Px(0.),
        Px(0.),
        Px(-5.),
        Px(5.),
    )
}

pub fn icon() -> impl Bundle {
    (
        Node {
            min_width: Px(24.),
            min_height: Px(24.),
            justify_content: JustifyContent::Center,
            border: UiRect::all(Px(2.)),
            ..default()
        },
        BorderColor(Color::linear_rgba(0., 0.1, 0.2, 0.75)),
        BackgroundColor(Srgba::hex("#ECF8FBFF").unwrap().into()),
        BorderRadius::all(Px(8.)),
    )
}

pub fn text(i18n: I18n) -> impl Bundle {
    (
        i18n,
        Text::default(),
        Observed::by(
            |trigger: Trigger<I18nNotify>, mut commands: Commands, fonts: Res<Fonts>| -> Result {
                trigger.format(
                    |key| trigger.arguments.get(key).map(String::as_str),
                    |string, style| {
                        commands.spawn((
                            ChildOf(trigger.target()),
                            TextSpan::new(string),
                            TextFont {
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
                            TextColor(style.color),
                        ));
                    },
                );

                Ok(())
            },
        ),
        Affected::by(|In(entity): In<Entity>, mut commands: Commands| {
            commands.entity(entity).queue(I18nNotifyCommand);
        }),
    )
}

pub fn scroll_text(i18n: I18n) -> impl Bundle {
    (
        i18n,
        Text::default(),
        ScrollText::default(),
        Observed::by(
            |trigger: Trigger<I18nNotify>,
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
        Affected::by(|In(entity): In<Entity>, mut commands: Commands| {
            commands.entity(entity).queue(I18nNotifyCommand);
        }),
    )
}

pub fn keyboard_binding(
    mut func: impl FnMut(&KeyboardBindings) -> KeyCode + 'static + Send + Sync,
) -> impl Bundle {
    (
        Text::default(),
        TextBounds::new(20., 20.),
        TextLayout::new(JustifyText::Center, LineBreak::NoWrap),
        Observed::by(
            move |trigger: Trigger<Rebind>,
                  mut text: Query<&mut Text>,
                  bindings: Res<Config<KeyboardBindings>>|
                  -> Result {
                let mut text = text.get_mut(trigger.target())?;

                let code = func(&bindings);
                keycode_desc(code)
                    .ok_or_else(|| format!("No unicode found for {code:?}"))?
                    .clone_into(&mut text);

                Ok(())
            },
        ),
        Affected::by(
            |In(entity): In<Entity>,
             mut commands: Commands,
             fonts: Res<Fonts>,
             mut query: Query<&mut TextFont>|
             -> Result {
                let mut font = query.get_mut(entity)?;
                font.font = fonts.bold.clone_weak();

                commands.entity(entity).trigger(Rebind);
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
