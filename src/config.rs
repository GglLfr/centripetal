use std::{
    fs, io,
    path::{Path, PathBuf},
};

use async_fs::File;
#[cfg(not(feature = "dev"))]
use bevy::asset::AsyncReadExt;
use bevy::{
    asset::{AsyncWriteExt, ron},
    prelude::*,
    tasks::{ConditionalSendFuture, IoTaskPool, Task, futures::check_ready},
    window::{PresentMode, PrimaryWindow, WindowMode, WindowResolution},
    winit::WinitWindows,
};
use blocking::unblock;
use directories::ProjectDirs;
use leafwing_input_manager::prelude::*;
use serde::{Deserialize, Serialize};

use crate::logic::{PlayerAction, entities::penumbra::AttractedAction};

#[derive(Debug, Clone, Resource)]
pub struct Dirs {
    pub data: PathBuf,
    pub settings: PathBuf,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct DirsPlugin;
impl Plugin for DirsPlugin {
    fn build(&self, app: &mut App) {
        let dirs = ProjectDirs::from("com.github", "GglLfr", "Centripetal")
            .or_else(|| ProjectDirs::from_path(PathBuf::from(".")))
            .expect("Couldn't get project directories");

        let data = PathBuf::from(dirs.data_dir());
        let settings = PathBuf::from(dirs.preference_dir());

        match fs::create_dir_all(&data).and_then(|()| fs::create_dir_all(&settings)) {
            Err(e) if e.kind() != io::ErrorKind::AlreadyExists => {
                panic!("Couldn't create config directories: {e}")
            }
            _ => {}
        }

        info!("Data directory: {}", data.display());
        info!("Settings directory: {}", settings.display());
        app.insert_resource(Dirs { data, settings });
    }
}

#[derive(Debug, Resource, Deref, DerefMut)]
pub struct Config<T: 'static + Send + Clone + Default + Serialize + for<'de> Deserialize<'de>> {
    #[deref]
    pub inner: T,
    path: PathBuf,
}

impl<T: 'static + Send + Clone + Default + Serialize + for<'de> Deserialize<'de>> Config<T> {
    pub fn new<P: Into<PathBuf>>(path: P) -> impl ConditionalSendFuture<Output = io::Result<Self>> + use<T, P> {
        let path = path.into();

        #[cfg(not(feature = "dev"))]
        {
            async move {
                let create_default = async |path: PathBuf| {
                    let (inner, inner_serialized) = unblock(|| {
                        let inner = T::default();
                        ron::ser::to_string_pretty(&inner, default()).map(|str| (inner, str))
                    })
                    .await
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

                    let mut file = File::create(&path).await?;
                    file.write_all(inner_serialized.as_bytes()).await?;
                    file.sync_all().await?;

                    Ok(Self { inner, path })
                };

                match File::open(&path).await {
                    Ok(mut file) => {
                        let mut bytes = Vec::new();
                        file.read_to_end(&mut bytes).await?;

                        match unblock(move || ron::de::from_bytes(&bytes)).await {
                            Ok(inner) => Ok(Self { inner, path }),
                            Err(e) => {
                                error!("Invalid config file {}: falling back to defaults!\n{e}", path.display());
                                create_default(path).await
                            }
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::NotFound => create_default(path).await,
                    Err(e) => Err(e),
                }
            }
        }

        #[cfg(feature = "dev")]
        {
            async move {
                Ok(Self {
                    inner: T::default(),
                    path,
                })
            }
        }
    }

    pub fn write(&self) -> impl ConditionalSendFuture<Output = io::Result<()>> + use<T> {
        let path = self.path.clone();
        let inner = self.inner.clone();
        async move {
            let serialized = unblock(move || ron::ser::to_string_pretty(&inner, default()))
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

            let mut file = File::create(&path).await?;
            file.write_all(serialized.as_bytes()).await?;
            file.sync_all().await?;

            Ok(())
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    pub present_mode: PresentMode,
    pub mode: WindowMode,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            present_mode: PresentMode::AutoVsync,
            #[cfg(not(feature = "dev"))]
            mode: WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            #[cfg(feature = "dev")]
            mode: WindowMode::Windowed,
        }
    }
}

impl WindowConfig {
    pub fn update_from(&mut self, window: &Window) {
        self.present_mode = window.present_mode;
        self.mode = window.mode;
    }

    pub fn update_to(&self, window: &mut Window) {
        window.present_mode = self.present_mode;
        window.mode = self.mode;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayerKeybinds {
    pub player: InputMap<PlayerAction>,
    pub attracted: InputMap<AttractedAction>,
}

impl Default for PlayerKeybinds {
    fn default() -> Self {
        Self {
            player: InputMap::default()
                .with_dual_axis(PlayerAction::Move, VirtualDPad::wasd())
                .with(PlayerAction::Attack, KeyCode::KeyZ),
            attracted: InputMap::default()
                .with_axis(
                    AttractedAction::Accel,
                    VirtualAxis::new(KeyCode::ControlLeft, KeyCode::ShiftLeft),
                )
                .with_axis(AttractedAction::Hover, VirtualAxis::vertical_arrow_keys())
                .with(AttractedAction::Precise, KeyCode::Space)
                .with(AttractedAction::Launch, KeyCode::KeyZ)
                .with(AttractedAction::Parry, KeyCode::KeyX),
        }
    }
}

#[derive(Debug, Resource)]
struct ConfigTask(Task<io::Result<(Config<WindowConfig>, Config<PlayerKeybinds>)>>);

#[derive(Debug, Copy, Clone, Default)]
pub struct ConfigPlugin;
impl Plugin for ConfigPlugin {
    fn build(&self, app: &mut App) {
        let dirs = app.world().resource::<Dirs>();
        let window = Config::new(dirs.settings.join("window.conf"));
        let keybinds = Config::new(dirs.settings.join("keybinds.conf"));

        app.insert_resource(ConfigTask(
            IoTaskPool::get().spawn(async move { Ok((window.await?, keybinds.await?)) }),
        ));
    }

    fn ready(&self, app: &App) -> bool {
        app.world().resource::<ConfigTask>().0.is_finished()
    }

    fn cleanup(&self, app: &mut App) {
        let world = app.world_mut();
        match check_ready(
            &mut world
                .remove_resource::<ConfigTask>()
                .expect("Missing `ConfigTask`; was it removed programmatically somewhere else?")
                .0,
        )
        .expect("`ready()` should ensure `ConfigTask::is_finished()`")
        {
            Ok((window_conf, keybind_conf)) => {
                #[cfg_attr(feature = "dev", expect(unused_mut))]
                let mut window = Window {
                    present_mode: window_conf.present_mode,
                    resolution: WindowResolution::new(1280., 800.),
                    title: "Centripetal".into(),
                    visible: false,
                    ..default()
                };

                #[cfg(not(feature = "dev"))]
                window.set_maximized(true);

                let id = world
                    .spawn((PrimaryWindow, window))
                    .observe(
                        |trigger: Trigger<OnRemove, Window>,
                         query: Query<&Window>,
                         mut config: ResMut<Config<WindowConfig>>|
                         -> Result {
                            let window = query.get(trigger.target())?;
                            config.update_from(window);

                            IoTaskPool::get().scope(|scope| scope.spawn(config.write()));
                            Ok(())
                        },
                    )
                    .id();

                let mode = window_conf.mode;
                world.insert_resource(window_conf);
                world.insert_resource(keybind_conf);
                world.flush();

                app.add_systems(
                    Startup,
                    move |mut query: Query<&mut Window>, windows: NonSend<WinitWindows>| -> Result {
                        let window = &mut *query.get_mut(id)?;
                        window.mode = mode;
                        window.visible = true;

                        if let WindowMode::Windowed = window.mode {
                            let window = windows
                                .entity_to_winit
                                .get(&id)
                                .and_then(|id| windows.windows.get(id))
                                .ok_or("No associated `winit` window found")?;

                            // Workaround for the split-second eye-blinding white screen on creation.
                            window.set_decorations(false);
                            window.set_visible(true);
                            window.set_decorations(true);
                        }

                        Ok(())
                    },
                );
            }
            Err(e) => panic!("Couldn't load config file(s): {e}"),
        }
    }
}
