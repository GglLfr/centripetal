use crate::{
    GameState, TileTextures,
    math::{GlobalTransform2d, Transform2d},
    prelude::*,
    render::{
        PIXELATED_LAYER,
        painter::{Blending, Painter, PainterParam},
    },
    world::{TILE_PIXEL_SIZE, Tilemap},
};

#[derive(Reflect, Component, Debug, Default, Clone, Copy)]
#[require(Transform2d::ABOVE, Painter, RenderLayers = PIXELATED_LAYER)]
#[reflect(Component, Debug, Default, FromWorld, Clone)]
pub struct TilemapGrid;

fn draw_tilemap_chunk_grid(
    param: PainterParam,
    textures: Res<TileTextures>,
    tilemaps: Query<(&Tilemap, &GlobalTransform2d, &Painter), With<TilemapGrid>>,
) {
    for (tilemap, &trns, painter) in tilemaps {
        let mut ctx = param.ctx(painter);
        for y in 0..tilemap.dimension().x {
            for x in 0..tilemap.dimension().y {
                ctx.blend = Blending::Additive;
                ctx.rect(
                    &textures.grid,
                    *trns * Affine2::from_translation(uvec2(x, y).as_vec2() * TILE_PIXEL_SIZE - Vec2::splat(0.5)),
                    (None, Anchor::BOTTOM_LEFT),
                );
            }
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.register_required_components::<Tilemap, TilemapGrid>().add_systems(
        PostUpdate,
        draw_tilemap_chunk_grid
            .after(TransformSystems::Propagate)
            .run_if(in_state(GameState::Editor)),
    );
}
