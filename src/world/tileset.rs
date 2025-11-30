use crate::{
    prelude::*,
    render::atlas::{AtlasInfo, AtlasRegion, PageInfo},
};

#[derive(Reflect, Debug)]
#[reflect(Debug)]
pub struct TilesetImage {
    pub region: Handle<AtlasRegion>,
    pub tiles: HashMap<UVec2, Handle<AtlasRegion>>,
}

impl Asset for TilesetImage {}
impl VisitAssetDependencies for TilesetImage {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        visit(self.region.id().untyped());
        for tile in self.tiles.values() {
            visit(tile.id().untyped());
        }
    }
}

pub struct TilesetImageLoader;
impl AssetLoader for TilesetImageLoader {
    type Asset = TilesetImage;
    type Settings = u32;
    type Error = BevyError;

    async fn load(&self, _: &mut dyn Reader, settings: &Self::Settings, load_context: &mut LoadContext<'_>) -> Result<Self::Asset, Self::Error> {
        let size = if *settings < 8 { Err(format!("Tileset grid size too small ({settings})"))? } else { *settings };
        let path = load_context.asset_path().clone();

        let region_asset = load_context.loader().immediate().load::<AtlasRegion>(path).await?;
        let region = region_asset.get();
        if region.rect.size() % size != UVec2::ZERO {
            Err(format!("Tileset image size ({}) isn't a multiple of {size}", region.rect.size()))?
        }

        let mut tiles = HashMap::new();
        for y in 0..region.rect.size().y / size {
            for x in 0..region.rect.size().x / size {
                tiles.insert(
                    uvec2(x, y),
                    load_context.add_labeled_asset(format!("{x},{y}"), AtlasRegion {
                        info: AtlasInfo {
                            page: PageInfo {
                                texture: region.page.texture.clone(),
                                texture_size: region.page.texture_size.clone(),
                            },
                            rect: URect {
                                min: region.rect.min + uvec2(x, y) * size,
                                max: region.rect.min + uvec2(x + 1, y + 1) * size,
                            },
                        },
                    }),
                );
            }
        }

        Ok(TilesetImage {
            region: load_context.add_loaded_labeled_asset("region", region_asset),
            tiles,
        })
    }

    fn extensions(&self) -> &[&str] {
        ImageLoader::SUPPORTED_FILE_EXTENSIONS
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_asset::<TilesetImage>()
        .register_asset_reflect::<TilesetImage>()
        .register_asset_loader(TilesetImageLoader);
}
