use crate::{
    logic::{
        CameraConfines, CameraTarget,
        entities::{
            Health, Killed, NoKillDespawn,
            penumbra::{AttractedPrediction, SeleneParry},
        },
        levels::penumbra_wing_l::{Instance, Respawned, SeleneUi, p1_spawn_attractor, spawn_selene},
    },
    prelude::*,
    suspend,
    ui::{ui_fade_in, ui_fade_out},
};

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
) {
    commands
        .entity(selene)
        // Make Selene "unkillable"; replace the default behavior with suspending and resuming instead.
        .insert(NoKillDespawn)
        .observe(
            move |trigger: Trigger<Killed>,
                  mut commands: Commands,
                  mut query: Query<(&Transform, &SeleneParry, &mut AttractedPrediction)>|
                  -> Result {
                let (&trns, &parry, mut prediction) = query.get_mut(trigger.target())?;
                prediction.points.clear();

                // Reset some states on death...
                commands
                    .entity(selene)
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
                commands.queue(spawn_selene(level_entity, selene, trns, selene_trns, || {}));
                Ok(())
            },
        )
        // Fade out UI during death and fade it back in on respawn.
        .observe(move |_: Trigger<Killed>, mut commands: Commands, mut ui: ResMut<SeleneUi>| {
            if let Some(ui) = ui.take()
                && let Ok(mut ui) = commands.get_entity(ui)
            {
                ui.queue(ui_fade_out);

                let ui_entity = ui.id();
                commands
                    .entity(selene)
                    .observe(move |trigger: Trigger<Respawned>, mut commands: Commands, mut ui: ResMut<SeleneUi>| {
                        commands.entity(trigger.observer()).despawn();
                        // Needs `is_none()` here to ensure we don't accidentally replace an existing UI.
                        // Such condition should be considered a bug; this is only a failsafe.
                        if ui.is_none()
                            && let Ok(mut e) = commands.get_entity(ui_entity)
                        {
                            e.queue(ui_fade_in);
                            **ui = Some(e.id());
                        }
                    });
            }
        });

    // Entry point.
    commands.entity(level_entity).observe(
        move |trigger: Trigger<OnRemove, p1_spawn_attractor::SpawningAttractor>, mut commands: Commands| {
            commands.entity(trigger.observer()).despawn();
            commands.entity(level_entity).insert(SpawningSelene);

            // Spawn Selene with an animation, and...
            commands.queue(spawn_selene(
                level_entity,
                selene,
                attractor_trns,
                selene_trns,
                move |mut commands: Commands| {
                    commands.entity(attractor).remove::<(CameraTarget, CameraConfines)>();
                    commands.entity(level_entity).remove::<SpawningSelene>();
                },
            ));
        },
    );
}
