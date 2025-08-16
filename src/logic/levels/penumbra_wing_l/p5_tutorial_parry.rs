use crate::{
    despawn, i18n,
    logic::levels::penumbra_wing_l::{Instance, p4_tutorial_launch},
    prelude::*,
    ui::BottomDialog,
};

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
pub struct TutorialParry;

pub fn init(
    InRef(&Instance { level_entity, .. }): InRef<Instance>,
    mut commands: Commands,
) -> Result {
    // Entry point.
    commands.entity(level_entity).observe(
        move |trigger: Trigger<OnRemove, p4_tutorial_launch::TutorialLaunch>,
              launches: Query<&p4_tutorial_launch::TutorialLaunch>,
              mut commands: Commands|
              -> Result {
            commands.queue(despawn(trigger.observer()));
            commands.entity(level_entity).insert(TutorialParry);

            // If we got the "special" outcome (obtained by not getting hit, which is by parrying), skip the
            // parrying tutorial section completely.
            let &launch = launches.get(level_entity)?;
            let (initial, _skip) = if let p4_tutorial_launch::TutorialLaunch::Normal = launch {
                (i18n!("tutorial.parry.enter.normal"), false)
            } else {
                (i18n!("tutorial.parry.enter.special"), true)
            };

            commands.queue(BottomDialog::show(
                initial,
                BottomDialog::show_next_after(
                    Duration::from_secs(2),
                    i18n!("tutorial.parry.recognition"),
                    |_: In<Entity>| {},
                ),
            ));

            Ok(())
        },
    );

    Ok(())
}
