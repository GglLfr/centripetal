use crate::prelude::*;

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct Movement;

#[derive(Reflect, Resource, Asset)]
pub struct Keybinds {
    //
}

#[derive(Component, Debug)]
#[require(RigidBody::Dynamic, LockedAxes::ROTATION_LOCKED)]
pub struct GroundedController {}

fn grounded_move(mut controllers: Query<(&GroundedController, Forces)>, movements: Query<(&Action<Movement>, &ActionOf<GroundedController>)>) {
    for (action, action_of) in movements {
        let Ok((_control, mut forces)) = controllers.get_mut(**action_of) else { continue };
        forces.apply_linear_acceleration(action.with_y(0.) * 80.);
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_input_context::<GroundedController>().add_systems(Update, grounded_move);
}
