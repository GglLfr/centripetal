use std::f32::consts::TAU;

use crate::{
    PIXELS_PER_UNIT, Sprites,
    graphics::{SpriteDrawer, SpriteSection},
    logic::{
        CameraConfines, CameraTarget, TimeFinished, Timed,
        effects::Ring,
        entities::{
            Health, Killed, NoKillDespawn,
            penumbra::{AttractedPrediction, SeleneParry},
        },
        levels::penumbra_wing_l::{Instance, p1_spawn_attractor},
    },
    math::{FloatTransformExt as _, Interp, RngExt as _},
    prelude::*,
    resume, suspend,
};

#[derive(Debug, Clone, Component, Default)]
#[require(SpriteDrawer, Timed::new(Duration::from_millis(2500)))]
pub struct SpawnEffect {
    target_pos: Vec2,
}

pub fn draw_spawn_effect(
    sprites: Res<Sprites>,
    sprite_sections: Res<Assets<SpriteSection>>,
    effects: Query<(Entity, &SpawnEffect, &SpriteDrawer, &Timed)>,
) {
    let rings @ [Some(..), Some(..), Some(..), Some(..), Some(..)] = [
        sprite_sections.get(&sprites.ring_2),
        sprite_sections.get(&sprites.ring_3),
        sprite_sections.get(&sprites.ring_4),
        sprite_sections.get(&sprites.ring_6),
        sprite_sections.get(&sprites.ring_8),
    ] else {
        return
    };

    let rings = rings.map(Option::unwrap);
    for (e, effect, drawer, &timed) in &effects {
        let mut rng = Rng::with_seed(e.to_bits());
        let f = timed.frac();

        let mut layer = -1f32;
        for (angle, vec) in rng
            .fork()
            .len_vectors(40, 0., TAU, 5. * PIXELS_PER_UNIT as f32, 10. * PIXELS_PER_UNIT as f32)
        {
            let ring = rings[rng.usize(0..rings.len())];
            let f_scl = f.threshold(0., rng.f32_within(0.75, 1.));

            let green = rng.f32_within(1., 2.);
            let blue = rng.f32_within(12., 24.);
            let alpha = rng.f32_within(0.5, 1.);

            let rotate = f_scl.threshold(0.4, 0.9).pow_in(2);
            let proceed = f_scl.threshold(0.25, 1.);
            let width = ring.size.x + (1. - f_scl.slope(0.5)).pow_in(6) * ring.size.x * 1.5;

            drawer.draw_at(
                (vec * f.pow_out(5)).lerp(effect.target_pos, proceed.pow_in(6)).extend(layer),
                angle.slerp(Rot2::radians((effect.target_pos - vec).to_angle()), rotate),
                ring.sprite_with(
                    Color::linear_rgba(1., green, blue, alpha * (1. - proceed.pow_in(7))),
                    vec2(width, ring.size.y),
                    Anchor::CenterRight,
                ),
            );

            layer = layer.next_down();
        }
    }
}

#[derive(Debug, Copy, Clone, Default, Event)]
pub struct Respawned;

#[must_use]
pub fn spawn_selene(
    level_entity: Entity,
    selene: Entity,
    effect_trns: Transform,
    selene_trns: Transform,
    accept: impl FnOnce(&mut EntityWorldMut) -> Result + 'static + Send,
) -> impl Command<Result> {
    let target_pos = GlobalTransform::from(selene_trns)
        .reparented_to(&GlobalTransform::from(effect_trns))
        .translation
        .xy();

    move |world: &mut World| -> Result {
        world
            .spawn((
                ChildOf(level_entity),
                effect_trns,
                Ring {
                    radius_to: 128.,
                    thickness_from: 2.,
                    colors: smallvec![Color::linear_rgb(1., 2., 6.), Color::linear_rgb(1., 1., 2.)],
                    radius_interp: Interp::PowOut { exponent: 2 },
                    ..default()
                },
                Timed::new(Duration::from_millis(640)),
            ))
            .observe(Timed::despawn_on_finished);

        accept(
            world
                .spawn((ChildOf(level_entity), SpawnEffect { target_pos }, effect_trns))
                .observe(Timed::despawn_on_finished)
                .observe(move |_: Trigger<TimeFinished>, mut commands: Commands| -> Result {
                    commands.get_entity(selene)?.queue(resume).trigger(Respawned);
                    Ok(())
                }),
        )
    }
}

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
pub struct SpawningSelene;

pub fn init(
    InRef(&Instance {
        level_entity,
        selene,
        selene_initial,
        selene_trns,
        attractor,
        attractor_trns,
        ..
    }): InRef<Instance>,
    mut commands: Commands,
) -> Result {
    // Make Selene "unkillable"; replace the default behavior with suspending and resuming instead.
    commands.entity(selene).insert(NoKillDespawn).observe(
        move |trigger: Trigger<Killed>, mut commands: Commands, mut query: Query<(&Transform, &SeleneParry, &mut AttractedPrediction)>| -> Result {
            let (&trns, &parry, mut prediction) = query.get_mut(trigger.target())?;
            prediction.points.clear();

            // Reset some states on death...
            commands
                .get_entity(selene)?
                .insert((
                    selene_trns,
                    selene_initial,
                    SeleneParry {
                        warn_time: default(),
                        ..parry
                    },
                    LinearVelocity::ZERO,
                    AngularVelocity::ZERO,
                    Health::new(10),
                ))
                .queue(suspend);

            // ...and respawn her with an animation.
            commands.queue(spawn_selene(level_entity, selene, trns, selene_trns, |_| Ok(())));

            Ok(())
        },
    );

    // Entry point.
    commands.entity(level_entity).observe(
        move |trigger: Trigger<OnRemove, p1_spawn_attractor::SpawningAttractor>, mut commands: Commands| {
            commands.entity(trigger.observer()).despawn();
            commands.entity(level_entity).insert(SpawningSelene);

            // Spawn Selene with an animation, and...
            commands.queue(spawn_selene(level_entity, selene, attractor_trns, selene_trns, move |e| {
                e.observe(move |_: Trigger<TimeFinished>, mut commands: Commands| -> Result {
                    // ...proceed to the next phase after she's spawned.
                    commands.get_entity(attractor)?.remove::<(CameraTarget, CameraConfines)>();
                    commands.get_entity(level_entity)?.remove::<SpawningSelene>();

                    Ok(())
                });
                Ok(())
            }));
        },
    );

    Ok(())
}
