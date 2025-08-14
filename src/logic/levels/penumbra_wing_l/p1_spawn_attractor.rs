use std::time::Duration;

use bevy::prelude::*;
use smallvec::smallvec;

use crate::{
    Sprites,
    graphics::{Animation, AnimationHooks, BaseColor},
    logic::{
        CameraConfines, CameraTarget, Timed, effects::Ring, levels::penumbra_wing_l::Instance,
    },
    math::Interp,
    resume,
};

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
pub struct SpawningAttractor;

pub fn init(
    InRef(&Instance {
        level_entity,
        attractor,
        attractor_trns,
        attractor_radius,
        ..
    }): InRef<Instance>,
    mut commands: Commands,
) -> Result {
    // Entry point.
    commands.get_entity(level_entity)?.insert(SpawningAttractor);

    // Delay 1 second...
    commands.spawn((
        ChildOf(level_entity),
        Timed::run(
            Duration::from_secs(1),
            move |_: In<Entity>, mut commands: Commands, sprites: Res<Sprites>| -> Result {
                // ...and then finally create the attractor spawn effectz
                commands
                    .get_entity(level_entity)?
                    .remove::<(CameraTarget, CameraConfines)>();

                commands.spawn((
                    ChildOf(level_entity),
                    Transform {
                        translation: attractor_trns.translation.with_z(1.),
                        ..attractor_trns
                    },
                    CameraTarget,
                    Animation::new(sprites.attractor_spawn.clone_weak(), "in"),
                    AnimationHooks::despawn_on_done("out")
                        .on_done("in", AnimationHooks::set("out", false))
                        .on_done(
                            "in",
                            move |_: In<Entity>,
                                  mut commands: Commands,
                                  sprites: Res<Sprites>|
                                  -> Result {
                                // Make the attractor visible.
                                commands
                                    .get_entity(attractor)?
                                    .queue(resume)
                                    .insert(CameraTarget)
                                    .with_children(move |children| {
                                        children.spawn((
                                            Transform::from_xyz(0., 0., 1.),
                                            Animation::new(
                                                sprites.grand_attractor_spawned.clone_weak(),
                                                "anim",
                                            ),
                                            AnimationHooks::despawn_on_done("anim"),
                                            BaseColor(Color::linear_rgba(1., 2., 24., 1.)),
                                        ));
                                    });

                                // Spawn an explosion ring effect.
                                commands
                                    .spawn((
                                        ChildOf(level_entity),
                                        attractor_trns,
                                        Ring {
                                            radius_from: attractor_radius,
                                            radius_to: attractor_radius + 24.,
                                            thickness_from: 2.,
                                            colors: smallvec![
                                                Color::linear_rgb(1., 2., 6.),
                                                Color::linear_rgb(1., 1., 2.)
                                            ],
                                            radius_interp: Interp::PowIn { exponent: 3 },
                                            ..default()
                                        },
                                        Timed::new(Duration::from_millis(480)),
                                    ))
                                    .observe(Timed::despawn_on_finished);

                                // Proceed to the next phase after the first sub-animation is done.
                                commands
                                    .get_entity(level_entity)?
                                    .remove::<SpawningAttractor>();

                                Ok(())
                            },
                        ),
                    BaseColor(Color::linear_rgba(1., 2., 24., 1.)),
                ));

                Ok(())
            },
        ),
    ));

    Ok(())
}
