/*#[cfg(not(feature = "dev"))]
use bevy::asset::AsyncReadExt;
use bevy::{
    asset::{AsyncWriteExt, ron},
    ecs::{component::HookContext, entity::EntityHashSet, world::DeferredWorld},
    prelude::*,
    tasks::{ConditionalSendFuture, IoTaskPool, Task, futures::check_ready},
    window::{PresentMode, PrimaryWindow, WindowMode, WindowResolution},
    winit::WinitWindows,
};
use blocking::unblock;
use directories::ProjectDirs;*/

use bevy::{
    asset::ron,
    tasks::{ConditionalSendFuture, IoTaskPool, Task, futures::check_ready},
    window::{PresentMode, PrimaryWindow, WindowMode, WindowResolution},
    winit::WinitWindows,
};
use blocking::unblock;
use directories::ProjectDirs;

use crate::{
    logic::{
        PlayerAction,
        entities::penumbra::{AttractedAction, LaunchAction},
    },
    prelude::*,
};

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
    pub fn new<P: Into<PathBuf>>(
        path: P,
    ) -> impl ConditionalSendFuture<Output = io::Result<Self>> + use<T, P> {
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
                                error!(
                                    "Invalid config file {}: falling back to defaults!\n{e}",
                                    path.display()
                                );
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

#[derive(Debug, Clone, Resource, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyboardBindings {
    pub player_attack: KeyCode,
    pub player_move: [KeyCode; 4],
    pub attracted_precise: KeyCode,
    pub attracted_parry: KeyCode,
    pub attracted_accel: [KeyCode; 2],
    pub attracted_hover: [KeyCode; 2],
    pub launch: KeyCode,
}

impl Default for KeyboardBindings {
    fn default() -> Self {
        Self {
            player_attack: KeyCode::KeyZ,
            player_move: [KeyCode::KeyW, KeyCode::KeyS, KeyCode::KeyA, KeyCode::KeyD],
            attracted_precise: KeyCode::Space,
            attracted_parry: KeyCode::KeyX,
            attracted_accel: [KeyCode::ControlLeft, KeyCode::ShiftLeft],
            attracted_hover: [KeyCode::ArrowDown, KeyCode::ArrowUp],
            launch: KeyCode::KeyZ,
        }
    }
}

impl KeyboardBindings {
    pub fn create_input_maps(
        &self,
    ) -> (
        InputMap<PlayerAction>,
        InputMap<AttractedAction>,
        InputMap<LaunchAction>,
    ) {
        (
            InputMap::new([(PlayerAction::Attack, self.player_attack)]).with_dual_axis(
                PlayerAction::Move,
                VirtualDPad::new(
                    self.player_move[0],
                    self.player_move[1],
                    self.player_move[2],
                    self.player_move[3],
                ),
            ),
            InputMap::new([
                (AttractedAction::Precise, self.attracted_precise),
                (AttractedAction::Parry, self.attracted_parry),
            ])
            .with_axis(
                AttractedAction::Accel,
                VirtualAxis::new(self.attracted_accel[0], self.attracted_accel[1]),
            )
            .with_axis(
                AttractedAction::Hover,
                VirtualAxis::new(self.attracted_hover[0], self.attracted_hover[1]),
            ),
            InputMap::new([(LaunchAction, self.launch)]),
        )
    }
}

#[derive(Debug, Clone, Default, Resource)]
pub struct RebindObservers(EntityHashSet);

#[derive(Debug, Copy, Clone, Component)]
#[component(on_add = on_rebind_add, on_remove = on_rebind_remove)]
pub struct RebindObserved;
fn on_rebind_add(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    world.resource_mut::<RebindObservers>().0.insert(entity);
}

fn on_rebind_remove(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    world.resource_mut::<RebindObservers>().0.remove(&entity);
}

#[derive(Debug, Copy, Clone, Default, Event)]
pub struct Rebind;

#[derive(Debug, Resource)]
struct ConfigTask(Task<io::Result<(Config<WindowConfig>, Config<KeyboardBindings>)>>);

pub fn keycode_desc(code: KeyCode) -> Option<&'static str> {
    match code {
        KeyCode::Unidentified(..) => None,
        KeyCode::Backquote => Some("`"),
        KeyCode::Backslash => Some("\\"),
        KeyCode::BracketLeft => Some("["),
        KeyCode::BracketRight => Some("]"),
        KeyCode::Comma => Some(","),
        KeyCode::Digit0 => Some("0"),
        KeyCode::Digit1 => Some("1"),
        KeyCode::Digit2 => Some("2"),
        KeyCode::Digit3 => Some("3"),
        KeyCode::Digit4 => Some("4"),
        KeyCode::Digit5 => Some("5"),
        KeyCode::Digit6 => Some("6"),
        KeyCode::Digit7 => Some("7"),
        KeyCode::Digit8 => Some("8"),
        KeyCode::Digit9 => Some("9"),
        KeyCode::Equal => Some("="),
        KeyCode::IntlBackslash => None,
        KeyCode::IntlRo => None,
        KeyCode::IntlYen => None,
        KeyCode::KeyA => Some("A"),
        KeyCode::KeyB => Some("B"),
        KeyCode::KeyC => Some("C"),
        KeyCode::KeyD => Some("D"),
        KeyCode::KeyE => Some("E"),
        KeyCode::KeyF => Some("F"),
        KeyCode::KeyG => Some("G"),
        KeyCode::KeyH => Some("H"),
        KeyCode::KeyI => Some("I"),
        KeyCode::KeyJ => Some("J"),
        KeyCode::KeyK => Some("K"),
        KeyCode::KeyL => Some("L"),
        KeyCode::KeyM => Some("M"),
        KeyCode::KeyN => Some("N"),
        KeyCode::KeyO => Some("O"),
        KeyCode::KeyP => Some("P"),
        KeyCode::KeyQ => Some("Q"),
        KeyCode::KeyR => Some("R"),
        KeyCode::KeyS => Some("S"),
        KeyCode::KeyT => Some("T"),
        KeyCode::KeyU => Some("U"),
        KeyCode::KeyV => Some("V"),
        KeyCode::KeyW => Some("W"),
        KeyCode::KeyX => Some("X"),
        KeyCode::KeyY => Some("Y"),
        KeyCode::KeyZ => Some("Z"),
        KeyCode::Minus => Some("-"),
        KeyCode::Period => Some("."),
        KeyCode::Quote => Some("\""),
        KeyCode::Semicolon => Some(";"),
        KeyCode::Slash => Some("/"),
        KeyCode::AltLeft => Some("Lᴀʟᴛ"),
        KeyCode::AltRight => Some("Rᴀʟᴛ"),
        KeyCode::Backspace => Some("⌫"),
        KeyCode::CapsLock => Some("⇪"),
        KeyCode::ContextMenu => None,
        KeyCode::ControlLeft => Some("Lᴄᴛʀʟ"),
        KeyCode::ControlRight => Some("Rᴄᴛʀʟ"),
        KeyCode::Enter => Some("↵"),
        KeyCode::SuperLeft => None,
        KeyCode::SuperRight => None,
        KeyCode::ShiftLeft => Some("Lꜱʜꜰᴛ"),
        KeyCode::ShiftRight => Some("Rꜱʜꜰᴛ"),
        KeyCode::Space => Some("Sᴘᴀᴄᴇ"),
        KeyCode::Tab => Some("⇥"),
        KeyCode::Convert => None,
        KeyCode::KanaMode => None,
        KeyCode::Lang1 => None,
        KeyCode::Lang2 => None,
        KeyCode::Lang3 => None,
        KeyCode::Lang4 => None,
        KeyCode::Lang5 => None,
        KeyCode::NonConvert => None,
        KeyCode::Delete => Some("⌦"),
        KeyCode::End => Some("↘"),
        KeyCode::Help => None,
        KeyCode::Home => Some("↖"),
        KeyCode::Insert => Some("Iɴꜱ"),
        KeyCode::PageDown => Some("Pɢᴅɴ"),
        KeyCode::PageUp => Some("Pɢᴜᴘ"),
        KeyCode::ArrowDown => Some("Dᴏᴡɴ"),
        KeyCode::ArrowLeft => Some("Lᴇꜰᴛ"),
        KeyCode::ArrowRight => Some("Rɪɢʜᴛ"),
        KeyCode::ArrowUp => Some("Uᴘ"),
        KeyCode::NumLock => None,
        KeyCode::Numpad0 => None,
        KeyCode::Numpad1 => None,
        KeyCode::Numpad2 => None,
        KeyCode::Numpad3 => None,
        KeyCode::Numpad4 => None,
        KeyCode::Numpad5 => None,
        KeyCode::Numpad6 => None,
        KeyCode::Numpad7 => None,
        KeyCode::Numpad8 => None,
        KeyCode::Numpad9 => None,
        KeyCode::NumpadAdd => None,
        KeyCode::NumpadBackspace => None,
        KeyCode::NumpadClear => None,
        KeyCode::NumpadClearEntry => None,
        KeyCode::NumpadComma => None,
        KeyCode::NumpadDecimal => None,
        KeyCode::NumpadDivide => None,
        KeyCode::NumpadEnter => None,
        KeyCode::NumpadEqual => None,
        KeyCode::NumpadHash => None,
        KeyCode::NumpadMemoryAdd => None,
        KeyCode::NumpadMemoryClear => None,
        KeyCode::NumpadMemoryRecall => None,
        KeyCode::NumpadMemoryStore => None,
        KeyCode::NumpadMemorySubtract => None,
        KeyCode::NumpadMultiply => None,
        KeyCode::NumpadParenLeft => None,
        KeyCode::NumpadParenRight => None,
        KeyCode::NumpadStar => None,
        KeyCode::NumpadSubtract => None,
        KeyCode::Escape => Some("Eꜱᴄ"),
        KeyCode::Fn => None,
        KeyCode::FnLock => None,
        KeyCode::PrintScreen => Some("PʀᴛSᴄ"),
        KeyCode::ScrollLock => None,
        KeyCode::Pause => None,
        KeyCode::BrowserBack => None,
        KeyCode::BrowserFavorites => None,
        KeyCode::BrowserForward => None,
        KeyCode::BrowserHome => None,
        KeyCode::BrowserRefresh => None,
        KeyCode::BrowserSearch => None,
        KeyCode::BrowserStop => None,
        KeyCode::Eject => None,
        KeyCode::LaunchApp1 => None,
        KeyCode::LaunchApp2 => None,
        KeyCode::LaunchMail => None,
        KeyCode::MediaPlayPause => None,
        KeyCode::MediaSelect => None,
        KeyCode::MediaStop => None,
        KeyCode::MediaTrackNext => None,
        KeyCode::MediaTrackPrevious => None,
        KeyCode::Power => None, // I can do the funniest thing ever.
        KeyCode::Sleep => None, // (things*)
        KeyCode::AudioVolumeDown => None,
        KeyCode::AudioVolumeMute => None,
        KeyCode::AudioVolumeUp => None,
        KeyCode::WakeUp => None,
        KeyCode::Meta => None,
        KeyCode::Hyper => None,
        KeyCode::Turbo => None,
        KeyCode::Abort => None,
        KeyCode::Resume => None,
        KeyCode::Suspend => None,
        KeyCode::Again => None,
        KeyCode::Copy => None,
        KeyCode::Cut => None,
        KeyCode::Find => None,
        KeyCode::Open => None,
        KeyCode::Paste => None,
        KeyCode::Props => None,
        KeyCode::Select => None,
        KeyCode::Undo => None,
        KeyCode::Hiragana => None,
        KeyCode::Katakana => None,
        KeyCode::F1 => None,
        KeyCode::F2 => None,
        KeyCode::F3 => None,
        KeyCode::F4 => None,
        KeyCode::F5 => None,
        KeyCode::F6 => None,
        KeyCode::F7 => None,
        KeyCode::F8 => None,
        KeyCode::F9 => None,
        KeyCode::F10 => None,
        KeyCode::F11 => None,
        KeyCode::F12 => None,
        KeyCode::F13 => None,
        KeyCode::F14 => None,
        KeyCode::F15 => None,
        KeyCode::F16 => None,
        KeyCode::F17 => None,
        KeyCode::F18 => None,
        KeyCode::F19 => None,
        KeyCode::F20 => None,
        KeyCode::F21 => None,
        KeyCode::F22 => None,
        KeyCode::F23 => None,
        KeyCode::F24 => None,
        KeyCode::F25 => None,
        KeyCode::F26 => None,
        KeyCode::F27 => None,
        KeyCode::F28 => None,
        KeyCode::F29 => None,
        KeyCode::F30 => None,
        KeyCode::F31 => None,
        KeyCode::F32 => None,
        KeyCode::F33 => None,
        KeyCode::F34 => None,
        KeyCode::F35 => None,
    }
}

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
                    move |mut query: Query<&mut Window>,
                          windows: NonSend<WinitWindows>|
                          -> Result {
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
