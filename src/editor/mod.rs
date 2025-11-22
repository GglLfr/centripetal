use crate::{GameState, prelude::*};

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
}

pub fn plugin(app: &mut App) {}
