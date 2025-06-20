use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_framepace::FramepacePlugin;

use crate::asset::SetupAssetPlugin;

pub mod asset;
mod config;
pub use config::*;

#[cfg_attr(not(feature = "dev"), global_allocator)]
#[cfg_attr(
    feature = "dev",
    expect(unused, reason = "Bevy dynamic linking is incompatible with Mimalloc redirection")
)]
static ALLOC: mimalloc_redirect::MiMalloc = mimalloc_redirect::MiMalloc;

fn main() -> AppExit {
    App::new()
        .insert_resource(ClearColor(Color::NONE))
        .add_plugins((
            DirsPlugin,
            DefaultPlugins.set(ImagePlugin::default_nearest()).set(WindowPlugin {
                // Set by `ConfigPlugin`.
                primary_window: None,
                ..default()
            }),
            ConfigPlugin,
            PhysicsPlugins::default(),
            #[cfg(feature = "dev")]
            PhysicsDebugPlugin::default(),
            FramepacePlugin,
            SetupAssetPlugin,
        ))
        .run()
}
