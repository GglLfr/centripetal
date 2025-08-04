use avian2d::parry::utils::hashmap::HashMap;
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_asset_loader::prelude::*;
use sys_locale::get_locales;

use crate::{
    I18nEntries, Locale,
    graphics::{SpriteSection, SpriteSheet},
    logic::Ldtk,
};

#[derive(Debug, Clone, Resource, AssetCollection, Deref)]
pub struct WorldHandle {
    #[asset(path = "levels/world.ldtk")]
    pub handle: Handle<Ldtk>,
}

#[derive(Debug, Clone, Resource, AssetCollection)]
pub struct Sprites {
    // Visual effects.
    #[asset(path = "effects/grand_attractor_spawned.json")]
    pub grand_attractor_spawned: Handle<SpriteSheet>,
    #[asset(path = "effects/ring_1.png")]
    pub ring_1: Handle<SpriteSection>,
    #[asset(path = "effects/ring_2.png")]
    pub ring_2: Handle<SpriteSection>,
    #[asset(path = "effects/ring_3.png")]
    pub ring_3: Handle<SpriteSection>,
    #[asset(path = "effects/ring_4.png")]
    pub ring_4: Handle<SpriteSection>,
    #[asset(path = "effects/ring_6.png")]
    pub ring_6: Handle<SpriteSection>,
    #[asset(path = "effects/ring_8.png")]
    pub ring_8: Handle<SpriteSection>,
    #[asset(path = "effects/ring_16.png")]
    pub ring_16: Handle<SpriteSection>,
    // Entities.
    // -- Attractor.
    #[asset(path = "entities/attractor/regular.json")]
    pub attractor_regular: Handle<SpriteSheet>,
    #[asset(path = "entities/attractor/slash.json")]
    pub attractor_slash: Handle<SpriteSheet>,
    #[asset(path = "entities/attractor/spawn.json")]
    pub attractor_spawn: Handle<SpriteSheet>,
    // -- Bullet.
    #[asset(path = "entities/bullet/spiky.json")]
    pub bullet_spiky: Handle<SpriteSheet>,
    // -- Generic.
    #[asset(path = "entities/generic/collectible_32.json")]
    pub collectible_32: Handle<SpriteSheet>,
    // -- Selene.
    #[asset(path = "entities/selene/selene.json")]
    pub selene: Handle<SpriteSheet>,
    #[asset(path = "entities/selene/selene_penumbra.json")]
    pub selene_penumbra: Handle<SpriteSheet>,
    #[asset(path = "entities/selene/try_launch_front.json")]
    pub selene_try_launch_front: Handle<SpriteSheet>,
    #[asset(path = "entities/selene/try_launch_back.json")]
    pub selene_try_launch_back: Handle<SpriteSheet>,
}

#[derive(Debug, Clone, Resource, AssetCollection)]
pub struct Fonts {
    #[asset(path = "fonts/bold_italic.ttf")]
    pub bold_italic: Handle<Font>,
    #[asset(path = "fonts/bold.ttf")]
    pub bold: Handle<Font>,
    #[asset(path = "fonts/italic.ttf")]
    pub italic: Handle<Font>,
    #[asset(path = "fonts/regular.ttf")]
    pub regular: Handle<Font>,
}

#[derive(Debug, Clone, Resource, Deref)]
pub struct Locales(pub HashMap<Locale, Handle<I18nEntries>>);
impl AssetCollection for Locales {
    fn load(world: &mut World) -> Vec<UntypedHandle> {
        let server = world.resource::<AssetServer>();
        get_locales()
            .filter_map(|s| {
                if Locale::from_bcp47(&s).is_some() {
                    Some(
                        server
                            .load::<I18nEntries>(format!("i18n/{s}.ron"))
                            .untyped(),
                    )
                } else {
                    None
                }
            })
            .collect()
    }

    fn create(world: &mut World) -> Self {
        let server = world.resource::<AssetServer>();
        Self(
            get_locales()
                .filter_map(|s| match Locale::from_bcp47(&s) {
                    Some(locale) => {
                        Some((locale, server.load::<I18nEntries>(format!("i18n/{s}.ron"))))
                    }
                    None => None,
                })
                .collect(),
        )
    }
}

#[derive(SystemParam)]
pub struct LocaleQuery<'w> {
    locales: Res<'w, Locales>,
    locale_assets: Res<'w, Assets<I18nEntries>>,
}

impl LocaleQuery<'_> {
    pub fn get(&self, locale: Locale) -> &I18nEntries {
        self.locale_assets.get(&self.locales[&locale]).unwrap()
    }
}
