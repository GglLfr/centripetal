use std::f32::consts::TAU;

use crate::{
    IntoResultSystem, PIXELS_PER_UNIT, SaveApp as _, Sprites,
    graphics::{SpriteDrawer, SpriteSection},
    logic::{
        Fields, FromLevel, LevelApp as _, LevelEntities, TimeFinished, Timed,
        effects::Ring,
        entities::penumbra::{AttractedInitial, Attractor, ThornRing},
        levels::{LevelTransitionSet, in_level},
    },
    math::{FloatTransformExt as _, Interp, RngExt as _},
    prelude::*,
    resume, suspend,
};

pub mod p1_spawn_attractor;
pub mod p2_spawn_selene;
pub mod p3_tutorial_align;
pub mod p4_tutorial_launch;
pub mod p5_tutorial_parry;

const SELENE: Uuid = uuid!("332e5310-3740-11f0-b0d1-4b444b848a1e");
const ATTRACTOR: Uuid = uuid!("8226eab0-3740-11f0-b0d1-31c3cf318fb2");
const RING: Uuid = uuid!("516847d0-3740-11f0-bea9-db42cbfffb80");
const HOVER_TARGET: Uuid = uuid!("ddc89020-3740-11f0-bea9-17dccf039850");

#[derive(Debug, Copy, Clone, Default, Resource, TypePath, Serialize, Deserialize, Deref, DerefMut)]
pub struct IntroShown(pub bool);

#[derive(Debug, Copy, Clone, Default, Deref, DerefMut, Resource)]
pub struct SeleneUi(pub Option<Entity>);

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
pub fn spawn_selene<M>(
    level_entity: Entity,
    selene: Entity,
    effect_trns: Transform,
    selene_trns: Transform,
    on_done: impl IntoResultSystem<(), (), M>,
) -> impl Command {
    let target_pos = GlobalTransform::from(selene_trns)
        .reparented_to(&GlobalTransform::from(effect_trns))
        .translation
        .xy();

    let mut on_done = IntoResultSystem::into_system(on_done);
    move |world: &mut World| {
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

        world.spawn((ChildOf(level_entity), SpawnEffect { target_pos }, effect_trns)).observe(
            move |trigger: Trigger<TimeFinished>, world: &mut World| -> Result {
                resume(world.get_entity_mut(selene)?);
                world.trigger_targets(Respawned, selene);
                world.flush();

                on_done.initialize(world);
                on_done.validate_param(world)?;
                on_done.run((), world)?;

                _ = world.try_despawn(trigger.target());
                world.flush();
                Ok(())
            },
        );
    }
}

#[derive(Debug, Component)]
pub struct Instance {
    pub level_entity: Entity,
    pub selene: Entity,
    pub attractor: Entity,
    pub ring: Entity,
    pub hover_target: Entity,
    pub selene_initial: AttractedInitial,
    pub attractor_radius: f32,
    pub selene_trns: Transform,
    pub attractor_trns: Transform,
    pub ring_radius: f32,
}

impl FromLevel for Instance {
    type Param = (
        SRes<IntroShown>,
        SQuery<Read<Transform>>,
        SQuery<Read<AttractedInitial>>,
        SQuery<Read<Attractor>>,
        SQuery<Read<ThornRing>>,
    );
    type Data = Read<LevelEntities>;

    fn from_level(
        mut e: EntityCommands,
        _: &Fields,
        (cutscene_shown, transforms, initials, attractors, rings): SystemParamItem<Self::Param>,
        entities: QueryItem<Self::Data>,
    ) -> Result {
        if **cutscene_shown {
            // TODO Can this level be revisited without the cutscene?
            return Ok(())
        }

        let level_entity = e.id();
        let mut commands = e.commands();
        let [selene, attractor, ring, hover_target] = [SELENE, ATTRACTOR, RING, HOVER_TARGET].map(|iid| {
            let e = entities.get(iid).unwrap();
            commands.entity(e).queue(suspend);

            e
        });

        let selene_initial = initials.get(selene).copied().unwrap_or_default();
        let attractor_radius = attractors.get(attractor)?.radius;
        let [&selene_trns, &attractor_trns] = transforms.get_many([selene, attractor])?;
        let ring_radius = rings.get(ring)?.radius;

        commands.init_resource::<SeleneUi>();
        commands.queue(move |world: &mut World| -> Result {
            let this = Self {
                level_entity,
                selene,
                attractor,
                ring,
                hover_target,
                selene_initial,
                attractor_radius,
                selene_trns,
                attractor_trns,
                ring_radius,
            };

            world.run_system_cached_with(p1_spawn_attractor::init, &this)??;
            world.run_system_cached_with(p2_spawn_selene::init, &this)??;
            world.run_system_cached_with(p3_tutorial_align::init, &this)??;
            world.run_system_cached_with(p4_tutorial_launch::init, &this)??;
            world.run_system_cached_with(p5_tutorial_parry::init, &this)??;

            world.get_entity_mut(level_entity)?.insert(this);
            Ok(())
        });

        Ok(())
    }
}

pub(super) fn plugin(app: &mut App) {
    app.register_level::<Instance>("penumbra_wing_l")
        .add_systems(
            PostUpdate,
            (draw_spawn_effect, p3_tutorial_align::update_align_time)
                .in_set(LevelTransitionSet)
                .run_if(in_level("penumbra_wing_l")),
        )
        .save_resource_init::<IntroShown>();
}
