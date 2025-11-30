use crate::{
    prelude::*,
    render::{
        atlas::AtlasRegion,
        painter::{Blending, Painter, PainterQuads, Vertex},
    },
};

#[derive(SystemParam)]
pub struct PainterParam<'w> {
    pub quads: Res<'w, PainterQuads>,
    pub regions: Res<'w, Assets<AtlasRegion>>,
}

impl<'a> PainterParam<'a> {
    pub fn ctx(&'a self, painter: &'a Painter) -> PainterContext<'a> {
        PainterContext {
            param: self,
            painter,
            blend: Blending::Normal,
            layer: 0.,
            color: LinearRgba::WHITE,
        }
    }
}

#[derive(Copy, Clone, Deref)]
pub struct PainterContext<'a> {
    #[deref]
    pub param: &'a PainterParam<'a>,
    pub painter: &'a Painter,
    pub blend: Blending,
    pub layer: f32,
    pub color: LinearRgba,
}

impl PainterContext<'_> {
    pub fn rect(self, region: impl Into<AssetId<AtlasRegion>>, trns: Affine2, (size, anchor): (Option<Vec2>, Anchor)) {
        let region = region.into();
        let Some(region) = self.regions.get(region) else {
            error!("Missing atlas region `{region}`");
            return
        };

        let size = size.unwrap_or(region.rect.size().as_vec2());
        let uvs = region.uv_corners();

        let bl = -size / 2. - *anchor * size;
        let tr = bl + size;

        self.quads.request(self.painter, &region.page.texture, self.blend, self.layer, [[
            Vertex::new(trns.transform_point2(bl), self.color, uvs[0]),
            Vertex::new(trns.transform_point2(vec2(tr.x, bl.y)), self.color, uvs[1]),
            Vertex::new(trns.transform_point2(tr), self.color, uvs[2]),
            Vertex::new(trns.transform_point2(vec2(bl.x, tr.y)), self.color, uvs[3]),
        ]]);
    }
}
