use bevy::{asset::load_internal_asset, prelude::*};
use bevy_asset_loader::prelude::*;
use bevy_vector_shapes::{
    render::{
        CORE_HANDLE, DISC_HANDLE, LINE_HANDLE, NGON_HANDLE, RECT_HANDLE, ShapeData as _,
        TRIANGLE_HANDLE,
    },
    shapes::{DiscData, LineData, NgonData, RectData, TriangleData},
};

use crate::{
    graphics::{SpriteSection, SpriteSheet},
    logic::Ldtk,
};

#[derive(Debug, Clone, Resource, AssetCollection, Deref, DerefMut)]
pub struct WorldHandle {
    #[asset(path = "levels/world.ldtk")]
    pub handle: Handle<Ldtk>,
}

#[derive(Debug, Clone, Resource, AssetCollection)]
pub struct Sprites {
    // Visual effects.
    #[asset(path = "effects/grand_attractor_spawned.json")]
    pub grand_attractor_spawned: Handle<SpriteSheet>,
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
}

#[derive(Debug, Clone, Resource, AssetCollection)]
pub struct Fonts {
    #[asset(path = "fonts/raleway/Raleway-VariableFont_wght.ttf")]
    pub raleway: Handle<Font>,
    #[asset(path = "fonts/raleway/Raleway-Italic-VariableFont_wght.ttf")]
    pub raleway_italic: Handle<Font>,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct ShapeShadersPlugin;
impl Plugin for ShapeShadersPlugin {
    fn build(&self, _app: &mut App) {}

    fn finish(&self, app: &mut App) {
        load_internal_asset!(
            app,
            CORE_HANDLE,
            "embedded_assets/shaders/core.wgsl",
            Shader::from_wgsl
        );

        let defs = DiscData::shader_defs(app);
        load_internal_asset!(
            app,
            DISC_HANDLE,
            "embedded_assets/shaders/shapes/src/render/shaders/shapes/disc.wgsl",
            Shader::from_wgsl_with_defs,
            defs
        );

        let defs = LineData::shader_defs(app);
        load_internal_asset!(
            app,
            LINE_HANDLE,
            "embedded_assets/shaders/shapes/src/render/shaders/shapes/line.wgsl",
            Shader::from_wgsl_with_defs,
            defs
        );

        let defs = NgonData::shader_defs(app);
        load_internal_asset!(
            app,
            NGON_HANDLE,
            "embedded_assets/shaders/shapes/src/render/shaders/shapes/ngon.wgsl",
            Shader::from_wgsl_with_defs,
            defs
        );

        let defs = RectData::shader_defs(app);
        load_internal_asset!(
            app,
            RECT_HANDLE,
            "embedded_assets/shaders/shapes/src/render/shaders/shapes/rect.wgsl",
            Shader::from_wgsl_with_defs,
            defs
        );

        let defs = TriangleData::shader_defs(app);
        load_internal_asset!(
            app,
            TRIANGLE_HANDLE,
            "embedded_assets/shaders/shapes/src/render/shaders/shapes/tri.wgsl",
            Shader::from_wgsl_with_defs,
            defs
        );
    }
}
