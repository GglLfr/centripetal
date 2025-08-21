use crate::{
    i18n,
    logic::{
        TimeFinished, Timed,
        entities::penumbra::LaunchAction,
        levels::penumbra_wing_l::{Instance, RingSpawnEffect, p5_tutorial_parry},
    },
    prelude::*,
    suspend,
    ui::BottomDialog,
};

pub fn init(
    InRef(&Instance {
        level_entity,
        selene,
        attractor_trns,
        ring,
        ring_radius,
        ..
    }): InRef<Instance>,
    mut commands: Commands,
) {
    // Entry point.
    commands.entity(level_entity).observe(
        move |trigger: Trigger<OnRemove, p5_tutorial_parry::TutorialParry>, mut commands: Commands| {
            commands.entity(trigger.observer()).despawn();

            // Some talk before enabling the launch action again.
            commands.queue(BottomDialog::show(
                None,
                i18n!("tutorial.end.leave.1"),
                BottomDialog::show_next_after(
                    Duration::from_secs(2),
                    i18n!("tutorial.end.leave.2"),
                    move |In(e): In<Entity>, mut commands: Commands| {
                        commands.spawn((
                            ChildOf(level_entity),
                            Timed::run(Duration::from_secs(2), move |mut commands: Commands| {
                                commands.queue(BottomDialog::hide(e));
                                commands
                                    .spawn((ChildOf(level_entity), attractor_trns, RingSpawnEffect { ring_radius }))
                                    .observe(
                                        move |_: Trigger<TimeFinished>,
                                              mut commands: Commands,
                                              mut actions: Query<&mut ActionState<LaunchAction>>|
                                              -> Result {
                                            commands.entity(ring).queue(suspend);
                                            commands.queue(BottomDialog::show(
                                                None,
                                                i18n!("tutorial.end.leave.3"),
                                                BottomDialog::hide_after(Duration::from_secs(2)),
                                            ));

                                            actions.get_mut(selene)?.enable_action(&LaunchAction);
                                            Ok(())
                                        },
                                    );
                            }),
                        ));
                    },
                ),
            ));
        },
    );
}
