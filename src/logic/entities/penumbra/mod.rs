use avian2d::{dynamics::solver::solver_body::SolverBody, prelude::*};
use bevy::prelude::*;

use crate::math::FloatTransformExt;

pub mod bullet;

mod attractor;
mod generic;
mod launchable;
mod selene_penumbra;
mod thorn_pillar;
mod thorn_ring;
pub use attractor::*;
pub use generic::*;
pub use launchable::*;
pub use selene_penumbra::*;
pub use thorn_pillar::*;
pub use thorn_ring::*;

#[derive(Debug, Copy, Clone, Default, Component)]
#[require(RigidBody::Kinematic)]
pub struct PenumbraEntity;

#[derive(Debug, Copy, Clone, Component, Deref, DerefMut)]
pub struct HomingTarget(pub Entity);

#[derive(Debug, Copy, Clone, Component, Deref, DerefMut)]
pub struct HomingPower(pub f32);

pub fn apply_homing_velocity(
    time: Res<Time>,
    mut homings: Query<(Entity, &HomingTarget, &HomingPower)>,
    mut bodies: Query<(&mut SolverBody, &Position)>,
) {
    let delta = time.delta_secs();
    for (e, &target, &power) in &mut homings {
        let Ok([(body, &pos), (other_body, &other_pos)]) = bodies.get_many_mut([e, *target]) else {
            continue;
        };

        let SolverBody {
            linear_velocity,
            delta_position,
            delta_rotation,
            ..
        } = body.into_inner();

        let target_angle = {
            let Some(Vec2 { x: cos, y: sin }) =
                (*other_pos + other_body.delta_position - (*pos + *delta_position)).try_normalize()
            else {
                continue;
            };
            Rotation { cos, sin }
        };

        let current_angle = {
            let Some(Vec2 { x: cos, y: sin }) = linear_velocity.try_normalize() else {
                continue;
            };
            Rotation { cos, sin }
        };

        let between = current_angle.angle_between(target_angle);
        let progress = Rotation::radians((power.copysign(between) * delta).min_mag(between));

        *linear_velocity = vec2(progress.cos, progress.sin).rotate(*linear_velocity);
        *delta_rotation *= progress;
    }
}
