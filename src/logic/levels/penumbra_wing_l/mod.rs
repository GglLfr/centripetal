use crate::{
    SaveApp as _,
    logic::{
        Fields, FromLevel, LevelApp as _, LevelEntities,
        entities::penumbra::{AttractedInitial, Attractor},
        levels::{LevelTransitionSet, in_level},
    },
    prelude::*,
    suspend,
};

pub mod p1_spawn_attractor;
pub mod p2_spawn_selene;
pub mod p3_tutorial_align;
pub mod p4_tutorial_launch;
pub mod p5_tutorial_parry;

const SELENE: Uuid = uuid!("332e5310-3740-11f0-b0d1-4b444b848a1e");
const ATTRACTOR: Uuid = uuid!("8226eab0-3740-11f0-b0d1-31c3cf318fb2");
const RINGS: [Uuid; 2] = [
    uuid!("483defc0-3740-11f0-bea9-1bca02df9366"),
    uuid!("516847d0-3740-11f0-bea9-db42cbfffb80"),
];
const HOVER_TARGET: Uuid = uuid!("ddc89020-3740-11f0-bea9-17dccf039850");

#[derive(Debug, Copy, Clone, Default, Resource, TypePath, Serialize, Deserialize, Deref, DerefMut)]
pub struct IntroShown(pub bool);

#[derive(Debug, Copy, Clone, Default, Deref, DerefMut, Resource)]
pub struct SeleneUi(pub Option<Entity>);

#[derive(Debug, Component)]
pub struct Instance {
    pub level_entity: Entity,
    pub selene: Entity,
    pub attractor: Entity,
    pub rings: [Entity; 2],
    pub hover_target: Entity,
    pub selene_initial: AttractedInitial,
    pub attractor_radius: f32,
    pub selene_trns: Transform,
    pub attractor_trns: Transform,
}

impl FromLevel for Instance {
    type Param = (
        SRes<IntroShown>,
        SQuery<Read<Transform>>,
        SQuery<Read<AttractedInitial>>,
        SQuery<Read<Attractor>>,
    );
    type Data = Read<LevelEntities>;

    fn from_level(
        mut e: EntityCommands,
        _: &Fields,
        (cutscene_shown, transforms, initials, attractors): SystemParamItem<Self::Param>,
        entities: QueryItem<Self::Data>,
    ) -> Result {
        if **cutscene_shown {
            // TODO Can this level be revisited without the cutscene?
            return Ok(());
        }

        let level_entity = e.id();
        let mut commands = e.commands();
        let [selene, attractor, ring_0, ring_1, hover_target] = [SELENE, ATTRACTOR, RINGS[0], RINGS[1], HOVER_TARGET].map(|iid| {
            let e = entities.get(iid).unwrap();
            commands.entity(e).queue(suspend);

            e
        });

        let selene_initial = initials.get(selene).copied().unwrap_or_default();
        let attractor_radius = attractors.get(attractor)?.radius;
        let [&selene_trns, &attractor_trns] = transforms.get_many([selene, attractor])?;

        commands.init_resource::<SeleneUi>();
        commands.queue(move |world: &mut World| -> Result {
            let this = Self {
                level_entity,
                selene,
                attractor,
                rings: [ring_0, ring_1],
                hover_target,
                selene_initial,
                attractor_radius,
                selene_trns,
                attractor_trns,
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
            (p2_spawn_selene::draw_spawn_effect, p3_tutorial_align::update_align_time)
                .in_set(LevelTransitionSet)
                .run_if(in_level("penumbra_wing_l")),
        )
        .save_resource_init::<IntroShown>();
}
