use std::{
    fs, io,
    path::{Path, PathBuf},
};

use async_fs::File;
use bevy::{
    asset::{AsyncReadExt, AsyncWriteExt, ron},
    prelude::*,
    tasks::{ConditionalSendFuture, IoTaskPool, Task, futures::check_ready},
    window::{PresentMode, PrimaryWindow, WindowMode, WindowResolution},
    winit::WinitWindows,
};
use blocking::unblock;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

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

                Ok(Config { inner, path })
            };

            match File::open(&path).await {
                Ok(mut file) => {
                    let mut bytes = Vec::new();
                    file.read_to_end(&mut bytes).await?;

                    match unblock(move || ron::de::from_bytes(&bytes)).await {
                        Ok(inner) => Ok(Config { inner, path }),
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
    width: u32,
    height: u32,
    position: WindowPosition,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 1024,
            height: 800,
            present_mode: PresentMode::AutoVsync,
            mode: WindowMode::Windowed,
            position: WindowPosition::Automatic,
        }
    }
}

impl WindowConfig {
    pub fn update_from(&mut self, window: &Window) {
        self.present_mode = window.present_mode;
        self.mode = window.mode;

        let UVec2 { x, y } = window.physical_size();
        self.width = x;
        self.height = y;
        self.position = if let pos @ WindowPosition::At(..) = window.position { pos } else { WindowPosition::Automatic };
    }

    pub fn update_to(&self, window: &mut Window) {
        window.present_mode = self.present_mode;
        window.mode = self.mode;
    }
}

#[derive(Debug, Resource)]
struct ConfigTask(Task<io::Result<(Config<WindowConfig>,)>>);

#[derive(Debug, Copy, Clone, Default)]
pub struct ConfigPlugin;
impl Plugin for ConfigPlugin {
    fn build(&self, app: &mut App) {
        let dirs = app.world().resource::<Dirs>();
        let window = Config::new(dirs.settings.join("window.conf"));

        app.insert_resource(ConfigTask(IoTaskPool::get().spawn(async move { Ok((window.await?,)) })));
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
            Ok((window_conf,)) => {
                let id = world
                    .spawn((PrimaryWindow, Window {
                        present_mode: window_conf.present_mode,
                        position: window_conf.position,
                        resolution: WindowResolution::new(window_conf.width as f32, window_conf.height as f32),
                        title: "Centripetal".into(),
                        visible: false,
                        ..default()
                    }))
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
