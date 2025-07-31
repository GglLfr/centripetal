use std::any::TypeId;

use bevy::{
    asset::load_internal_asset,
    core_pipeline::core_2d::Transparent2d,
    prelude::*,
    render::{
        Render, RenderApp, RenderSet, render_asset::prepare_assets, render_phase::DrawFunctions,
        texture::GpuImage,
    },
};
use bevy_vector_shapes::{
    render::{DrawShape2dCommand, ShapeData},
    shapes::{DiscData, LineData, NgonData, RectData, TriangleData},
};
use seldom_state::set::StateSet;

mod animation;
mod color;
mod fbo;
mod shape;
mod sprite_alloc;
mod sprite_drawer;
mod sprite_sheet;

pub use animation::*;
pub use color::*;
pub use fbo::*;
pub use shape::*;
pub use sprite_alloc::*;
pub use sprite_drawer::*;
pub use sprite_sheet::*;

#[derive(Debug, Copy, Clone, Default)]
pub struct GraphicsPlugin;
impl Plugin for GraphicsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<SpriteSection>()
            .init_asset::<SpriteSheet>()
            .add_systems(
                PostUpdate,
                (
                    update_animations.before(StateSet::Transition),
                    draw_animations,
                    flush_drawer_to_children,
                )
                    .chain()
                    .before(TransformSystem::TransformPropagate),
            );

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<PendingSprites>()
                .add_systems(ExtractSchedule, extract_pending_sprites)
                .add_systems(
                    Render,
                    prepare_pending_sprites.after(prepare_assets::<GpuImage>),
                );
        }
    }

    fn finish(&self, app: &mut App) {
        let (sender, receiver) = async_channel::bounded(8);
        app.init_resource::<SpriteAllocator>()
            .register_asset_loader(SpriteSheetLoader(sender.clone()))
            .register_asset_loader(SpriteSectionLoader(sender))
            .add_systems(PreUpdate, pack_incoming_sprites(receiver));

        load_internal_asset!(app, SHAPE_SHADER, "pixelized_shape.wgsl", Shader::from_wgsl);

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            fn setup<T: ShapeData>(world: &mut World) {
                let function = FboWrappedDraw::<
                    Transparent2d,
                    DrawShape2dCommand<T>,
                    BlitPixelizedShapes,
                >::new(world);

                let mut functions = world.resource::<DrawFunctions<Transparent2d>>().write();
                if let Some(&index) = functions
                    .indices
                    .get(&TypeId::of::<DrawShape2dCommand<T>>())
                {
                    let actual = functions
                        .get_mut(index)
                        .map(|draw| {
                            &*draw as *const dyn bevy::render::render_phase::Draw<Transparent2d>
                        })
                        .unwrap();

                    let mut found = false;
                    for draw in &mut functions.draw_functions {
                        if std::ptr::addr_eq(&**draw, actual) {
                            *draw = Box::new(function);
                            found = true;
                            break;
                        }
                    }

                    if !found {
                        panic!(
                            "DrawShape2dCommand<{}> not found",
                            std::any::type_name::<T>()
                        )
                    }
                } else {
                    functions.add_with::<DrawShape2dCommand<T>, _>(function);
                }
            }

            let world = render_app
                .init_resource::<LockedTextureCache>()
                .init_resource::<BlitPixelizedShapes>()
                .add_systems(
                    Render,
                    (
                        (
                            prepare_blit_pixelized_shape_buffers,
                            prepare_blit_pixelized_shape_pipelines,
                        )
                            .in_set(RenderSet::Prepare),
                        update_locked_texture_cache.in_set(RenderSet::Cleanup),
                    ),
                )
                .world_mut();

            setup::<DiscData>(world);
            setup::<LineData>(world);
            setup::<NgonData>(world);
            setup::<RectData>(world);
            setup::<TriangleData>(world);
        }
    }
}
