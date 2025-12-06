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
        let half_size = size / 2.;
        let center = -*anchor * size;
        let [uv0, uv1, uv2, uv3] = region.uv_corners();

        let bl = center - half_size;
        let tr = center + half_size;

        self.quads.request(self.painter, &region.page.texture, self.blend, self.layer, [[
            Vertex::new(trns.transform_point2(vec2(bl.x, bl.y)), self.color, uv0),
            Vertex::new(trns.transform_point2(vec2(tr.x, bl.y)), self.color, uv1),
            Vertex::new(trns.transform_point2(vec2(tr.x, tr.y)), self.color, uv2),
            Vertex::new(trns.transform_point2(vec2(bl.x, tr.y)), self.color, uv3),
        ]]);
    }

    pub fn line(self, region: impl Into<AssetId<AtlasRegion>>, from: Vec2, from_thickness: f32, to: Vec2, to_thickness: f32) {
        let region = region.into();
        let Some(region) = self.regions.get(region) else {
            error!("Missing atlas region `{region}`");
            return
        };

        let [uv0, uv1, uv2, uv3] = region.uv_corners();

        let Some([cos, sin]) = (to - from).try_normalize().map(|v| v.to_array()) else { return };
        let bias = vec2(sin, -cos);
        let bias_from = bias * from_thickness / 2.;
        let bias_to = bias * to_thickness / 2.;

        self.quads.request(self.painter, &region.page.texture, self.blend, self.layer, [[
            Vertex::new(from + bias_from, self.color, uv0),
            Vertex::new(from - bias_from, self.color, uv1),
            Vertex::new(to - bias_to, self.color, uv2),
            Vertex::new(to + bias_to, self.color, uv3),
        ]]);
    }
}
