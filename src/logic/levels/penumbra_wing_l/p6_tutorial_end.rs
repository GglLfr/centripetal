use crate::{
    i18n,
    logic::levels::penumbra_wing_l::{Instance, p5_tutorial_parry},
    prelude::*,
    ui::BottomDialog,
};

pub fn init(InRef(&Instance { level_entity, .. }): InRef<Instance>, mut commands: Commands) {
    // Entry point.
    commands.entity(level_entity).observe(
        move |trigger: Trigger<OnRemove, p5_tutorial_parry::TutorialParry>, mut commands: Commands| {
            commands.entity(trigger.observer()).despawn();

            commands.queue(BottomDialog::show(None, i18n!("tutorial.end.leave"), |_: In<Entity>| {}));
        },
    );
}
