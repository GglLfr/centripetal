mod attribute;
pub use attribute::*;

pub mod animation;
pub mod atlas;
pub mod painter;

use crate::{math::Transform2d, prelude::*};

pub const MAIN_LAYER: RenderLayers = RenderLayers::layer(0);
pub const OUTPUT_LAYER: RenderLayers = RenderLayers::layer(1);

#[derive(Component, Reflect, Debug, Default, Clone, Copy)]
#[require(Transform2d)]
#[reflect(Debug, Default, FromWorld, Clone)]
pub struct CameraTarget {
    pub priority: i32,
}

#[derive(Component, Reflect, Debug, Default, Clone, Copy)]
#[reflect(Component, Debug, Default, FromWorld, Clone)]
pub struct MainCamera {
    pub pos: Vec2,
}

impl MainCamera {
    pub fn snapped_pos(self) -> Vec2 {
        self.pos.round()
    }
}

#[derive(Component, Reflect, Debug, Default, Clone, Copy)]
#[reflect(Component, Debug, Default, FromWorld, Clone)]
pub struct OutputCamera;

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
        Msaa::Off,
        MainCamera::default(),
        MAIN_LAYER,
    ));

    commands.spawn((Camera2d, Hdr, IsDefaultUiCamera, OutputCamera, OUTPUT_LAYER));
    commands.spawn((Sprite::from_image(image), PixelatedCanvas, OUTPUT_LAYER));
}

fn update_canvas(
    window: Single<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    pixelated_camera: Single<&Camera, With<MainCamera>>,
    main_camera: Single<&Transform, (With<OutputCamera>, Without<PixelatedCanvas>)>,
    mut pixelated_canvas: Single<&mut Transform, With<PixelatedCanvas>>,
) {
    if let RenderTarget::Image(ImageRenderTarget { handle, .. }) = &pixelated_camera.target
        && let Some(canvas_image) = images.get_mut_untracked(handle)
    {
        let size = Extent3d {
            width: (window.physical_width() / 4).max(2),
            height: (window.physical_height() / 4).max(2),
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
        scale: trns.scale * 4.,
        ..trns
    };
}

fn move_camera_to_target(
    targets: Query<(Entity, &CameraTarget)>,
    transforms: Query<(&Transform2d, Option<&ChildOf>)>,
    mut camera_trns: Single<&mut MainCamera>,
) {
    let Some(target) = targets.into_iter().max_by_key(|(.., target)| target.priority).map(|(entity, ..)| entity) else { return };
    let Ok((trns, mut child_of)) = transforms.get(target) else { return };

    let mut trns = *trns;
    while let Some(is_child_of) = child_of
        && let Ok((parent_trns, parent_child_of)) = transforms.get(is_child_of.parent())
    {
        trns = *parent_trns * trns;
        child_of = parent_child_of;
    }

    camera_trns.pos = trns.translation.truncate();
}

fn snap_camera(camera_trns: Single<(&MainCamera, &mut Transform)>) {
    let (&camera, mut trns) = camera_trns.into_inner();
    trns.translation = camera.pos.extend(trns.translation.z);
}

pub fn plugin(app: &mut App) {
    use bevy::transform::systems::*;

    app.add_plugins((animation::plugin, atlas::plugin, painter::plugin))
        .add_systems(Startup, spawn_cameras)
        .add_systems(Update, update_canvas)
        .add_systems(
            PostUpdate,
            (move_camera_to_target, snap_camera)
                .chain()
                .before(mark_dirty_trees)
                .in_set(TransformSystems::Propagate),
        );
}
