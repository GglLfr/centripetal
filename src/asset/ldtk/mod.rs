use bevy::{
    asset::{UntypedAssetId, VisitAssetDependencies, uuid::Uuid},
    platform::collections::HashMap,
    prelude::*,
};

mod loader;

pub use loader::*;

#[derive(Debug, Clone, TypePath)]
pub struct Ldtk {
    pub iid: Uuid,
    pub bg_color: Srgba,
    pub levels: HashMap<Uuid, Handle<LdtkLevel>>,
}

impl Asset for Ldtk {}
impl VisitAssetDependencies for Ldtk {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        self.levels.values().for_each(|handle| visit(handle.id().untyped()));
    }
}

#[derive(Debug, Asset, TypePath)]
pub struct LdtkLevel {
    pub iid: Uuid,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct LdtkPlugin;
impl Plugin for LdtkPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Ldtk>()
            .init_asset::<LdtkLevel>()
            .init_asset_loader::<LdtkLoader>()
            .init_asset_loader::<LdtkLevelLoader>();
    }
}
