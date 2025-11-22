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
    image_loader: ImageLoader,
}

impl AssetLoader for AtlasRegionLoader {
    type Asset = AtlasRegion;
    type Settings = ImageLoaderSettings;
    type Error = BevyError;

    async fn load(&self, reader: &mut dyn Reader, settings: &Self::Settings, load_context: &mut LoadContext<'_>) -> Result<Self::Asset, Self::Error> {
        let image = self.image_loader.load(reader, settings, load_context).await?;
        let info = self.requester.request(image).await?;
        Ok(AtlasRegion { info })
    }

    fn extensions(&self) -> &[&str] {
        ImageLoader::SUPPORTED_FILE_EXTENSIONS
    }
}

fn init_atlas_region_loader(server: Res<AssetServer>, device: Res<RenderDevice>, requesters: Res<AtlasRequesters>) {
    server.register_loader(AtlasRegionLoader {
        requester: requesters.new_sender(),
        image_loader: ImageLoader::new(CompressedImageFormats::from_features(device.features())),
    });
}

pub(crate) fn plugin(app: &mut App) {
    app.init_asset::<AtlasRegion>()
        .register_asset_reflect::<AtlasRegion>()
        .preregister_asset_loader::<AtlasRegionLoader>(ImageLoader::SUPPORTED_FILE_EXTENSIONS)
        .add_systems(Startup, init_atlas_region_loader);
}
