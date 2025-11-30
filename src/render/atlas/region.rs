use crate::{
    prelude::*,
    render::atlas::{AtlasInfo, AtlasRequester, AtlasRequesters},
};

#[derive(Reflect, Asset, Debug, Clone, Deref, DerefMut)]
#[reflect(Debug, Clone)]
pub struct AtlasRegion {
    pub info: AtlasInfo,
}

#[derive(Clone)]
pub struct AtlasRegionLoader {
    requester: AtlasRequester,
}

impl AssetLoader for AtlasRegionLoader {
    type Asset = AtlasRegion;
    type Settings = ();
    type Error = BevyError;

    async fn load(&self, _: &mut dyn Reader, _: &Self::Settings, load_context: &mut LoadContext<'_>) -> Result<Self::Asset, Self::Error> {
        let path = load_context.asset_path().clone();
        let image = load_context.loader().immediate().load(path).await?;
        let info = self.requester.request(image.take()).await?;
        Ok(AtlasRegion { info })
    }

    fn extensions(&self) -> &[&str] {
        ImageLoader::SUPPORTED_FILE_EXTENSIONS
    }
}

fn init_atlas_region_loader(server: Res<AssetServer>, requesters: Res<AtlasRequesters>) {
    server.register_loader(AtlasRegionLoader {
        requester: requesters.new_sender(),
    });
}

pub(crate) fn plugin(app: &mut App) {
    app.init_asset::<AtlasRegion>()
        .register_asset_reflect::<AtlasRegion>()
        .preregister_asset_loader::<AtlasRegionLoader>(ImageLoader::SUPPORTED_FILE_EXTENSIONS)
        .add_systems(Startup, init_atlas_region_loader);
}
