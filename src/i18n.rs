use std::{
    borrow::Cow,
    fmt::{self, Debug},
    io,
    str::FromStr,
};

use bevy::{
    asset::{
        AssetLoader, AsyncReadExt, LoadContext,
        io::Reader,
        ron::{self, de::SpannedError},
    },
    ecs::{component::HookContext, entity::EntityHashSet, world::DeferredWorld},
    platform::collections::HashMap,
    prelude::*,
};
use derive_more::{Display, Error, From, FromStr};
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::{alpha1, alphanumeric1, hex_digit1, space0, u32},
    combinator::{cut, map, map_res, recognize, value, verify},
    error::{ErrorKind, ParseError, context},
    multi::{fold_many0, many0_count, separated_list0},
    sequence::{delimited, pair, preceded},
};
use nom_language::error::{VerboseError, convert_error};
use serde::{
    Deserialize, Deserializer,
    de::{self, Visitor},
};
use smallvec::SmallVec;
use sys_locale::get_locale;

use crate::Locales;

#[macro_export]
macro_rules! i18n {
    ($key:expr $(, $name:ident = $named:expr)* $(,)?) => {
        $crate::I18n {
            key: ::std::borrow::Cow::from($key),
            arguments: [$((::std::borrow::Cow::Borrowed(stringify!($name)), ::std::string::ToString::to_string($named))),*].into_iter().collect(),
        }
    };
}

#[derive(Debug, Clone, Default, Asset, TypePath, Deref, DerefMut)]
pub struct I18nEntries(pub HashMap<String, I18nEntry>);

#[derive(Debug, Display, Error, From)]
pub enum I18nEntriesError {
    Io(io::Error),
    Ron(SpannedError),
}

#[derive(Debug, Copy, Clone, Default)]
pub struct I18nEntriesLoader;
impl AssetLoader for I18nEntriesLoader {
    type Asset = I18nEntries;
    type Settings = ();
    type Error = I18nEntriesError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &Self::Settings,
        _: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut input = String::new();
        reader.read_to_string(&mut input).await?;

        Ok(I18nEntries(ron::from_str(&input)?))
    }

    fn extensions(&self) -> &[&str] {
        &["ron"]
    }
}

#[derive(Debug, Copy, Clone, FromStr, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Locale {
    #[default]
    EnUS,
}

impl Locale {
    pub fn from_bcp47(bcp: impl AsRef<str>) -> Option<Self> {
        Self::from_str(&Self::bcp47_to_ident(bcp)).ok()
    }

    pub fn bcp47_to_ident(bcp: impl AsRef<str>) -> String {
        let mut bcp = bcp.as_ref().chars();
        let mut output = String::with_capacity(4);
        output.push(bcp.next().unwrap().to_ascii_uppercase());
        output.push(bcp.next().unwrap());
        assert_eq!(bcp.next(), Some('-'));
        output.push(bcp.next().unwrap());
        output.push(bcp.next().unwrap());
        assert_eq!(bcp.count(), 0);
        output
    }
}

#[derive(Debug, Clone, Resource)]
pub struct I18nContext {
    current_locale: Locale,
    listeners: EntityHashSet,
    just_added: Vec<Entity>,
}

impl Default for I18nContext {
    fn default() -> Self {
        Self {
            current_locale: get_locale()
                .and_then(Locale::from_bcp47)
                .unwrap_or(Locale::EnUS),
            listeners: default(),
            just_added: Vec::new(),
        }
    }
}

pub fn i18n_notify_just_added(mut ctx: ResMut<I18nContext>, mut commands: Commands) {
    for added in ctx.just_added.drain(..) {
        if let Ok(mut e) = commands.get_entity(added) {
            e.queue(I18nNotify);
        }
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct I18nNotify;
impl EntityCommand<Result> for I18nNotify {
    fn apply(self, mut entity: EntityWorldMut) -> Result {
        let Some(I18n { key, arguments }) = entity.get::<I18n>().cloned() else {
            return Ok(());
        };

        let locale = entity.resource::<I18nContext>().current_locale;
        let handle = entity.resource::<Locales>()[&locale].clone_weak();
        let fmt = entity
            .resource::<Assets<I18nEntries>>()
            .get(&handle)
            .unwrap()
            .get(&*key)
            .ok_or(format!("I18n key `{key}` does not exist"))?
            .clone();

        entity.trigger(OnI18nNotify {
            locale,
            fmt,
            arguments,
        });
        Ok(())
    }
}

#[derive(Debug, Clone, Event, Deref, DerefMut)]
pub struct OnI18nNotify {
    pub locale: Locale,
    #[deref]
    pub fmt: I18nEntry,
    pub arguments: HashMap<Cow<'static, str>, String>,
}

#[derive(Debug, Clone, Component)]
#[component(on_add = i18n_on_add, on_remove = i18n_on_remove)]
pub struct I18n {
    pub key: Cow<'static, str>,
    pub arguments: HashMap<Cow<'static, str>, String>,
}

fn i18n_on_add(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    let mut ctx = world.resource_mut::<I18nContext>();
    ctx.listeners.insert(entity);
    ctx.just_added.push(entity);

    world.commands().entity(entity).queue(I18nNotify);
}

fn i18n_on_remove(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    world
        .resource_mut::<I18nContext>()
        .listeners
        .remove(&entity);
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct I18nEntry {
    format: String,
    contexts: SmallVec<[(usize, I18nFormatCtx); 4]>,
}

impl I18nEntry {
    pub fn format<'a>(
        &'a self,
        mut argument: impl FnMut(&'a str) -> Option<&'a str>,
        mut accept: impl FnMut(&'a str, I18nStyle),
    ) {
        let mut style = I18nStyle::default();
        let mut string = self.format.as_str();
        for &(offset, ref ctx) in &self.contexts {
            let (slice, new_string) = string.split_at(offset);
            string = new_string;

            if !slice.is_empty() {
                accept(slice, style);
            }

            match ctx {
                &I18nFormatCtx::Style(new_style) => style = new_style,
                I18nFormatCtx::Argument(arg) => {
                    if let Some(arg) = argument(arg) {
                        accept(arg, style);
                    }
                }
            }
        }

        if !string.is_empty() {
            accept(string, style);
        }
    }
}

impl FromStr for I18nEntry {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match parse_i18n_fmt(s) {
            Ok((.., fmt)) => Ok(fmt),
            Err(nom::Err::Error(e) | nom::Err::Failure(e)) => Err(convert_error(s, e)),
            Err(nom::Err::Incomplete(..)) => {
                unreachable!("`I18nFmt` parsing uses the complete variant")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum I18nFormatCtx {
    Style(I18nStyle),
    Argument(String),
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct I18nStyle {
    pub bold: bool,
    pub italic: bool,
    pub size: u32,
    pub color: Color,
}

impl Default for I18nStyle {
    fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            size: 20,
            color: Color::WHITE,
        }
    }
}

impl<'de> Deserialize<'de> for I18nEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visit;
        impl<'de> Visitor<'de> for Visit {
            type Value = I18nEntry;

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                I18nEntry::from_str(v).map_err(E::custom)
            }

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                writeln!(formatter, "a I18n bundle string")
            }
        }

        deserializer.deserialize_str(Visit)
    }
}

type I18nResult<'a, T> = IResult<&'a str, T, VerboseError<&'a str>>;

fn parse_i18n_plain<'a>(input: &'a str) -> I18nResult<'a, &'a str> {
    context(
        "I18n plain",
        verify(is_not("[]{}"), |s: &str| !s.is_empty()),
    )
    .parse(input)
}

fn parse_i18n_style_inner<'a>(input: &'a str) -> I18nResult<'a, I18nStyle> {
    #[derive(Copy, Clone)]
    enum Style {
        Bold,
        Italic,
        Size(u32),
        Color(Color),
    }

    context(
        "I18n style",
        map(
            separated_list0(
                delimited(space0, tag(","), space0),
                alt((
                    value(Style::Bold, tag("b")),
                    value(Style::Italic, tag("i")),
                    map(
                        preceded(
                            preceded(tag("s"), cut(delimited(space0, tag(":"), space0))),
                            cut(u32),
                        ),
                        Style::Size,
                    ),
                    map_res(
                        preceded(
                            preceded(tag("c"), cut(delimited(space0, tag(":"), space0))),
                            cut(preceded(tag("#"), hex_digit1)),
                        ),
                        |s| {
                            let color = Srgba::hex(s)?;
                            Ok::<_, HexColorError>(Style::Color(color.into()))
                        },
                    ),
                )),
            ),
            |attribs| {
                let mut style = I18nStyle::default();
                for attrib in attribs {
                    match attrib {
                        Style::Bold => style.bold = true,
                        Style::Italic => style.italic = true,
                        Style::Size(size) => style.size = size,
                        Style::Color(color) => style.color = color,
                    }
                }
                style
            },
        ),
    )
    .parse(input)
}

fn parse_i18n_style_outer<'a>(input: &'a str) -> I18nResult<'a, I18nStyle> {
    context(
        "I18n style braces",
        delimited(tag("["), cut(parse_i18n_style_inner), cut(tag("]"))),
    )
    .parse(input)
}

fn parse_i18n_argument<'a>(input: &'a str) -> I18nResult<'a, String> {
    context(
        "I18n arg",
        map(
            delimited(
                tag("{"),
                cut(recognize(pair(
                    alt((alpha1, tag("_"))),
                    many0_count(alt((alphanumeric1, tag("_")))),
                ))),
                cut(tag("}")),
            ),
            String::from,
        ),
    )
    .parse(input)
}

fn parse_i18n_escaped<'a>(input: &'a str) -> I18nResult<'a, char> {
    context(
        "I18nFmt escaped",
        alt((
            // Don't cut the opening braces because it might be a style or an argument.
            value('[', tag("[[")),
            value('{', tag("{{")),
            // Styles and arguments handle closing braces so cut it here.
            value(']', preceded(tag("]"), cut(tag("]")))),
            value('}', preceded(tag("}"), cut(tag("}")))),
        )),
    )
    .parse(input)
}

fn parse_i18n_end<'a, T>(input: &'a str) -> I18nResult<'a, T> {
    let e = VerboseError::from_error_kind(input, ErrorKind::Eof);
    if input.is_empty() {
        Err(nom::Err::Error(e))
    } else {
        Err(nom::Err::Failure(e))
    }
}

fn parse_i18n_fmt<'a>(input: &'a str) -> I18nResult<'a, I18nEntry> {
    enum Fmt<'a> {
        Slice(&'a str),
        Char(char),
        Segment(I18nFormatCtx),
    }

    context(
        "I18nFmt",
        map(
            cut(fold_many0(
                alt((
                    map(parse_i18n_escaped, Fmt::Char),
                    map(parse_i18n_argument, |arg| {
                        Fmt::Segment(I18nFormatCtx::Argument(arg))
                    }),
                    map(parse_i18n_style_outer, |style| {
                        Fmt::Segment(I18nFormatCtx::Style(style))
                    }),
                    map(parse_i18n_plain, Fmt::Slice),
                    parse_i18n_end,
                )),
                || (0, I18nEntry::default()),
                |(mut fmt_len, mut fmt), input| {
                    match input {
                        Fmt::Slice(str) => {
                            fmt_len += str.len();
                            fmt.format.push_str(str);
                        }
                        Fmt::Char(c) => {
                            fmt_len += c.len_utf8();
                            fmt.format.push(c);
                        }
                        Fmt::Segment(segment) => fmt
                            .contexts
                            .push((std::mem::replace(&mut fmt_len, 0), segment)),
                    }

                    (fmt_len, fmt)
                },
            )),
            |(.., fmt)| fmt,
        ),
    )
    .parse(input)
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;
    use smallvec::smallvec;
    use std::str::FromStr;

    use crate::{I18nEntry, I18nFormatCtx, I18nStyle};

    #[test]
    fn parse_i18n() {
        fn expect(test: Result<I18nEntry, String>, actual: I18nEntry) {
            match test {
                Ok(test) => assert_eq!(test, actual),
                Err(error) => panic!("{error}"),
            }
        }

        expect(
            I18nEntry::from_str("Hello [b, i, s: 128, c: #ABCDEF]{name}[]!"),
            I18nEntry {
                format: "Hello !".into(),
                contexts: smallvec![
                    (
                        6,
                        I18nFormatCtx::Style(I18nStyle {
                            bold: true,
                            italic: true,
                            size: 128,
                            color: Srgba::hex("#ABCDEF").unwrap().into(),
                        }),
                    ),
                    (0, I18nFormatCtx::Argument("name".into())),
                    (0, I18nFormatCtx::Style(default())),
                ],
            },
        );
    }
}
