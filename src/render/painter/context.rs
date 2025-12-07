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

impl Debug for PainterParam<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct AssetsWrapper;
        impl Debug for AssetsWrapper {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct("Assets<AtlasRegion>").finish_non_exhaustive()
            }
        }

        f.debug_struct("PainterParam")
            .field("quads", &self.quads)
            .field("regions", &AssetsWrapper)
            .finish()
    }
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

#[derive(Debug, Copy, Clone, Deref)]
pub struct PainterContext<'a> {
    #[deref]
    pub param: &'a PainterParam<'a>,
    pub painter: &'a Painter,
    pub blend: Blending,
    pub layer: f32,
    pub color: LinearRgba,
}

impl<'a> PainterContext<'a> {
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

    pub fn quad(self, region: impl Into<AssetId<AtlasRegion>>, vertices: [Vec2; 4]) {
        let region = region.into();
        let Some(region) = self.regions.get(region) else {
            error!("Missing atlas region `{region}`");
            return
        };

        let [uv0, uv1, uv2, uv3] = region.uv_corners();
        self.quads.request(self.painter, &region.page.texture, self.blend, self.layer, [[
            Vertex::new(vertices[0], self.color, uv0),
            Vertex::new(vertices[1], self.color, uv1),
            Vertex::new(vertices[2], self.color, uv2),
            Vertex::new(vertices[3], self.color, uv3),
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

    pub fn polyline(self, region: impl Into<AssetId<AtlasRegion>>) -> Polyline<'a> {
        Polyline {
            ctx: self,
            region: region.into(),
            data: PolylineData::Init,
        }
    }
}

// TODO The rendering is broken, and I couldn't be bothered to fix it right now.
#[derive(Debug)]
pub struct Polyline<'a> {
    ctx: PainterContext<'a>,
    region: AssetId<AtlasRegion>,
    data: PolylineData,
}

impl Polyline<'_> {
    pub fn point(&mut self, at: Vec2, width: f32) -> &mut Self {
        use PolylineData::*;
        self.data = match self.data {
            Init => Primary {
                start_pos: at,
                start_width: width,
            },
            Primary { start_pos, start_width } => Secondary {
                start_pos,
                start_width,
                next_pos: at,
                next_width: width,
            },
            Secondary {
                start_pos,
                start_width,
                next_pos,
                next_width,
            } => {
                let mid_width = path_join(start_pos, next_pos, at, next_width);
                Tertiary {
                    start_pos,
                    start_width,
                    start_post: (next_pos, mid_width),
                    tail_pos: at,
                    tail_width: width,
                }
            }
            Tertiary {
                start_pos,
                start_width,
                start_post: start_post @ (start_post_pos, start_post_width),
                tail_pos,
                tail_width,
            } => {
                let mid_width = path_join(start_post_pos, tail_pos, at, tail_width);
                self.ctx.quad(self.region, [
                    start_post_pos - start_post_width,
                    start_post_pos + start_post_width,
                    tail_pos + mid_width,
                    tail_pos - mid_width,
                ]);

                Quaternary {
                    start_pos,
                    start_width,
                    start_post,
                    prev: (tail_pos, mid_width),
                    tail_pos: at,
                    tail_width: width,
                }
            }
            Quaternary {
                start_pos,
                start_width,
                start_post,
                prev: (prev_pos, prev_width),
                tail_pos,
                tail_width,
            } => {
                let mid_width = path_join(prev_pos, tail_pos, at, tail_width);
                self.ctx.quad(self.region, [
                    prev_pos - prev_width,
                    prev_pos + prev_width,
                    tail_pos + mid_width,
                    tail_pos - mid_width,
                ]);

                Quaternary {
                    start_pos,
                    start_width,
                    start_post,
                    prev: (tail_pos, mid_width),
                    tail_pos: at,
                    tail_width: width,
                }
            }
        };
        self
    }

    pub fn finish(self, wrap: bool) {
        Self::finish_inner(self.ctx, self.region, self.data, wrap);
        mem::forget(self);
    }

    #[inline]
    fn finish_inner(ctx: PainterContext<'_>, region: AssetId<AtlasRegion>, data: PolylineData, wrap: bool) {
        use PolylineData::*;
        match data {
            Init | Primary { .. } => {}
            Secondary {
                start_pos,
                start_width,
                next_pos,
                next_width,
            } => {
                ctx.line(region, start_pos, start_width, next_pos, next_width);
            }
            Tertiary {
                start_pos,
                start_width,
                start_post: (start_post_pos, start_post_width),
                tail_pos,
                tail_width,
            } => {
                let (start_width, tail_width) = if wrap {
                    let start_width = path_join(tail_pos, start_pos, start_post_pos, start_width / 2.);
                    let tail_width = path_join(start_post_pos, tail_pos, start_pos, tail_width / 2.);

                    ctx.quad(region, [
                        tail_pos - tail_width,
                        tail_pos + tail_width,
                        start_pos + start_width,
                        start_pos - start_width,
                    ]);

                    (start_width, tail_width)
                } else {
                    (
                        -path_end(start_post_pos, start_pos, start_width / 2.),
                        path_end(start_post_pos, tail_pos, tail_width / 2.),
                    )
                };

                ctx.quad(region, [
                    start_pos - start_width,
                    start_pos + start_width,
                    start_post_pos + start_post_width,
                    start_post_pos - start_post_width,
                ]);

                ctx.quad(region, [
                    start_post_pos - start_post_width,
                    start_post_pos + start_post_width,
                    tail_pos + tail_width,
                    tail_pos - tail_width,
                ]);
            }
            Quaternary {
                start_pos,
                start_width,
                start_post: (start_post_pos, start_post_width),
                prev: (prev_pos, prev_width),
                tail_pos,
                tail_width,
            } => {
                let (start_width, tail_width) = if wrap {
                    let start_width = path_join(tail_pos, start_pos, start_post_pos, start_width / 2.);
                    let tail_width = path_join(start_post_pos, tail_pos, start_pos, tail_width / 2.);

                    ctx.quad(region, [
                        tail_pos - tail_width,
                        tail_pos + tail_width,
                        start_pos + start_width,
                        start_pos - start_width,
                    ]);

                    (start_width, tail_width)
                } else {
                    (
                        -path_end(start_post_pos, start_pos, start_width / 2.),
                        path_end(prev_pos, tail_pos, tail_width / 2.),
                    )
                };

                ctx.quad(region, [
                    start_pos - start_width,
                    start_pos + start_width,
                    start_post_pos + start_post_width,
                    start_post_pos - start_post_width,
                ]);

                ctx.quad(region, [
                    prev_pos - prev_width,
                    prev_pos + prev_width,
                    tail_pos + tail_width,
                    tail_pos - tail_width,
                ]);
            }
        }
    }
}

impl Drop for Polyline<'_> {
    fn drop(&mut self) {
        error!(
            "{}`Polyline` dropped without calling `finish()` first; finishing in non-wrapping mode",
            MaybeLocation::caller().map(|loc| format!("{loc}: ")).unwrap_or_default()
        );
        Self::finish_inner(self.ctx, self.region, self.data, false);
    }
}

#[derive(Debug, Clone, Copy)]
enum PolylineData {
    /// Just started; no data yet.
    Init,
    /// Start at this point, keeping the value for wrapping if configured to do so.
    Primary { start_pos: Vec2, start_width: f32 },
    /// Two vertices; can't start drawing, because the angles can't be determined yet.
    Secondary {
        start_pos: Vec2,
        start_width: f32,
        next_pos: Vec2,
        next_width: f32,
    },
    /// Three vertices; can't start drawing, because the angle for wrapping can't be determined yet.
    Tertiary {
        start_pos: Vec2,
        start_width: f32,
        start_post: (Vec2, Vec2),
        tail_pos: Vec2,
        tail_width: f32,
    },
    /// Four vertices; can definitely draw.
    Quaternary {
        start_pos: Vec2,
        start_width: f32,
        start_post: (Vec2, Vec2),
        prev: (Vec2, Vec2),
        tail_pos: Vec2,
        tail_width: f32,
    },
}

fn path_join(a: Vec2, b: Vec2, c: Vec2, hw: f32) -> Vec2 {
    let v1 = b - a;
    let v2 = c - b;

    let t = vec2(v2.x * v1.y - v2.y * v1.x, v1.x * v2.x + v1.y * v2.y).to_angle();
    if !t.is_finite() {
        Vec2::ZERO
    } else if t.abs() <= 1e-5 {
        let v = v1.normalize_or_zero() * hw;
        vec2(v.y, -v.x)
    } else {
        let len = hw / t.sin();
        let v1 = v1.normalize_or_zero() * len;
        let v2 = v2.normalize_or_zero() * len;

        match t < 0. {
            false => v2 - v1,
            true => v1 - v2,
        }
    }
}

fn path_end(start: Vec2, end: Vec2, hw: f32) -> Vec2 {
    let v = (end - start).normalize_or_zero() * hw;
    vec2(-v.y, v.x)
}
