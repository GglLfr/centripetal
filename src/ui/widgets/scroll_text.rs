use crate::prelude::*;

#[derive(Debug, Clone, Component, Default)]
#[require(TextLayout, ScrollTextState)]
pub struct ScrollText {
    pub sections: Vec<ScrollTextSection>,
}

impl ScrollText {
    pub fn new(sections: impl IntoIterator<Item = ScrollTextSection>) -> Self {
        Self {
            sections: sections.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ScrollTextSection {
    pub span: String,
    pub font: TextFont,
    pub color: TextColor,
    pub time_per_char: Duration,
}

impl ScrollTextSection {
    pub fn new(
        span: impl Into<String>,
        font: TextFont,
        color: impl Into<TextColor>,
        time_per_char: Duration,
    ) -> Self {
        Self {
            span: span.into(),
            font,
            color: color.into(),
            time_per_char,
        }
    }
}

#[derive(Debug, Clone, Component, Default)]
pub struct ScrollTextState {
    pub span_index: usize,
    pub span_byte_index: usize,
    pub time: Duration,
    pub done: bool,
}

#[derive(Debug, Copy, Clone, Default, Event)]
pub struct ScrollTextFinished;

pub fn update_scroll_text_sections(
    time: Res<Time>,
    mut commands: Commands,
    mut scrolls: Query<(Entity, &ScrollText, &mut ScrollTextState, Option<&Children>)>,
    mut spans: Query<&mut TextSpan>,
) {
    let delta = time.delta();
    for (e, text, mut state, children) in &mut scrolls {
        let mut spans = spans.iter_many_mut(children.map(Children::deref).unwrap_or(&[]));
        for _ in 0..state.span_index {
            spans.fetch_next();
        }

        state.time += delta;
        loop {
            let Some(section) = text.sections.get(state.span_index) else {
                if !std::mem::replace(&mut state.done, true) {
                    commands.entity(e).trigger(ScrollTextFinished);
                }

                break;
            };

            state.done = false;

            let mut slice = &section.span[state.span_byte_index..];
            while state.time >= section.time_per_char
                && let Some(c) = slice.chars().next()
            {
                state.time -= section.time_per_char;

                state.span_byte_index += c.len_utf8();
                slice = &section.span[c.len_utf8()..];
            }

            if let Some(span) = spans.fetch_next() {
                span.map_unchanged(DerefMut::deref_mut)
                    .set_if_neq(section.span[0..state.span_byte_index].to_owned());
            } else {
                commands.spawn((
                    ChildOf(e),
                    TextSpan(section.span[0..state.span_byte_index].into()),
                    section.font.clone(),
                    section.color,
                ));
            }

            if slice.is_empty() {
                state.span_index += 1;
                state.span_byte_index = 0;
            } else if state.time < section.time_per_char {
                break;
            }
        }

        while let Some(span) = spans.fetch_next() {
            span.map_unchanged(DerefMut::deref_mut)
                .set_if_neq(String::new());
        }
    }
}
