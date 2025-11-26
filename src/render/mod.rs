mod attribute;
pub use attribute::*;

pub mod atlas;
pub mod painter;

use crate::prelude::*;

pub const PIXEL_SIZE: u32 = 4;
pub const PIXELATED_LAYER: RenderLayers = RenderLayers::layer(0);
pub const MAIN_LAYER: RenderLayers = RenderLayers::layer(1);

#[derive(Component, Reflect, Debug, Default, Clone, Copy)]
#[reflect(Component, Debug, Default, FromWorld, Clone)]
pub struct PixelatedCamera;

#[derive(Component, Reflect, Debug, Default, Clone, Copy)]
#[reflect(Component, Debug, Default, FromWorld, Clone)]
pub struct MainCamera;

#[derive(Component, Reflect, Debug, Default, Clone, Copy)]
#[reflect(Component, Debug, Default, FromWorld, Clone)]
pub struct PixelatedCanvas;

fn spawn_cameras(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let image = images.add(Image::new_target_texture(2, 2, ViewTarget::TEXTURE_FORMAT_HDR));
    commands.spawn((
        Camera2d,
        Camera {
            order: -1,
            target: RenderTarget::from(image.clone()),
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        Hdr,
        Msaa::Sample2,
        PixelatedCamera,
        PIXELATED_LAYER,
    ));

    commands.spawn((Camera2d, Hdr, IsDefaultUiCamera, MainCamera, MAIN_LAYER));
    commands.spawn((Sprite::from_image(image), PixelatedCanvas, MAIN_LAYER));
}

fn update_canvas(
    window: Single<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    pixelated_camera: Single<&Camera, With<PixelatedCamera>>,
    main_camera: Single<&Transform, (With<MainCamera>, Without<PixelatedCanvas>)>,
    mut pixelated_canvas: Single<&mut Transform, With<PixelatedCanvas>>,
) {
    if let RenderTarget::Image(ImageRenderTarget { handle, .. }) = &pixelated_camera.target
        && let Some(canvas_image) = images.get_mut_untracked(handle)
    {
        let size = Extent3d {
            width: (window.physical_width() / PIXEL_SIZE).max(2),
            height: (window.physical_height() / PIXEL_SIZE).max(2),
            depth_or_array_layers: 1,
        };

        if canvas_image.texture_descriptor.size != size {
            canvas_image.resize(size);
            images.get_mut(handle).expect("Notifying change event");
        }
    }

    let trns = **main_camera;
    **pixelated_canvas = Transform {
        translation: trns.translation.with_z(0.),
        scale: trns.scale * PIXEL_SIZE as f32,
        ..trns
    };
}

pub fn plugin(app: &mut App) {
    app.add_plugins((atlas::plugin, painter::plugin))
        .add_systems(Startup, spawn_cameras)
        .add_systems(Update, update_canvas);
}
