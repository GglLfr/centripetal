use bevy::{
    asset::{LoadState, RecursiveDependencyLoadState, uuid::Uuid},
    prelude::*,
};
use iyes_progress::ProgressEntry;

use crate::{
    asset::{
        LdtkWorld,
        ldtk::{LdtkLayerData, LdtkLevel},
    },
    logic::InGameState,
};

#[derive(Debug, Copy, Clone, Event, Deref, DerefMut)]
pub struct LoadLevelEvent(pub Uuid);

#[derive(Debug, Clone, Component, Deref, DerefMut)]
#[require(LevelSpawned)]
pub struct LevelHandle(pub Handle<LdtkLevel>);

#[derive(Debug, Copy, Clone, Component, Default)]
pub struct LevelSpawned(bool);

#[derive(Debug, Clone, Component)]
pub struct LevelLayer {
    pub id: String,
}

impl LevelLayer {
    pub const MAIN: &'static str = "layer_main";
}

pub fn handle_load_level_event(
    mut commands: Commands,
    mut events: EventReader<LoadLevelEvent>,
    server: Res<AssetServer>,
    world: LdtkWorld,
    mut state: ResMut<NextState<InGameState>>,
) -> Result {
    let Some(&event) = events.read().last() else { return Ok(()) };
    commands.spawn(LevelHandle(
        server.load(
            world
                .levels
                .get(&*event)
                .ok_or_else(|| format!("Level {} not found", event.as_hyphenated()))?,
        ),
    ));

    state.set(InGameState::Loading);
    Ok(())
}

pub fn handle_load_level_progress(
    mut commands: Commands,
    tracker: ProgressEntry<InGameState>,
    server: Res<AssetServer>,
    mut level_handles: Query<(Entity, &LevelHandle, &mut LevelSpawned)>,
    levels: Res<Assets<LdtkLevel>>,
) -> Result {
    let mut all_done = 0;
    let mut all_total = 0;

    for (e, handle, mut spawned) in &mut level_handles {
        let mut done = false;
        match (
            server.load_state(handle.id()),
            server.recursive_dependency_load_state(handle.id()),
        ) {
            (LoadState::NotLoaded, ..) => Err("The level's asset handle is dropped")?,
            (LoadState::Loading, ..) | (.., RecursiveDependencyLoadState::Loading) => {}
            (LoadState::Loaded, RecursiveDependencyLoadState::NotLoaded) => {
                Err("The level's asset dependency handle is dropped")?
            }
            (LoadState::Loaded, RecursiveDependencyLoadState::Loaded) => done = true,
            (LoadState::Failed(e), ..) | (.., RecursiveDependencyLoadState::Failed(e)) => Err(e)?,
        }

        if done && !std::mem::replace(&mut spawned.0, true) {
            let level = levels.get(handle.id()).ok_or("Level asset unloaded")?;
            commands.insert_resource(ClearColor(level.bg_color));

            commands.entity(e).with_children(|children| {
                for layer in &level.layers {
                    children
                        .spawn(LevelLayer { id: layer.id.clone() })
                        .with_children(|layer_children| match &layer.data {
                            LdtkLayerData::IntGrid { grid, tiles } => {
                                for &val in grid {
                                    layer_children.spawn(val);
                                }

                                if let Some(tiles) = tiles {
                                    //
                                }
                            }
                        });
                }
            });
        }

        all_done += if done { 1 } else { 0 };
        all_total += 1;
    }

    tracker.set_progress(all_done, all_total);
    Ok(())
}
