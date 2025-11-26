use crate::{GameState, prelude::*, world::Tilemap};

pub fn start(mut commands: Commands, mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::Editor);
    commands.spawn((
        DespawnOnExit(GameState::Editor),
        Node {
            width: percent(100),
            height: percent(100),
            ..default()
        },
        children![],
    ));

    commands.spawn(Tilemap::new(uvec2(128, 128)));
}

pub fn plugin(app: &mut App) {}
