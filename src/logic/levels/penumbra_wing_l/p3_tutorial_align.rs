use crate::{
    Sprites,
    graphics::{Animation, AnimationMode, BaseColor},
    logic::{
        Timed,
        effects::Ring,
        levels::penumbra_wing_l::{Instance, p2_spawn_selene},
    },
    prelude::*,
};

const TUTORIAL_MOVE_ALIGN_HELP: Duration = Duration::from_millis(500);
const TUTORIAL_MOVE_ALIGN_DURATION: Duration = Duration::from_secs(5);

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
pub struct TutorialAlign {
    time: Duration,
    within: bool,
}

pub fn init(
    InRef(&Instance {
        level_entity,
        hover_target,
        ..
    }): InRef<Instance>,
    mut commands: Commands,
    shapes: ShapeCommands,
    sprites: Res<Sprites>,
) -> Result {
    commands.entity(hover_target).insert((
        Collider::circle(8.),
        CollisionEventsEnabled,
        Animation::new(sprites.collectible_32.clone_weak(), "anim"),
        AnimationMode::Repeat,
        BaseColor(Color::linear_rgb(12., 2., 1.)),
        DiscComponent::arc(shapes.config(), 16., 0., 0.),
        ShapeMaterial {
            alpha_mode: ShapeAlphaMode::Blend,
            disable_laa: false,
            pipeline: ShapePipelineType::Shape2d,
            canvas: None,
            texture: None,
        },
        ShapeFill {
            color: Color::linear_rgb(4., 2., 1.),
            ty: FillType::Stroke(1.5, ThicknessType::World),
        },
        Timed::repeat(
            Duration::from_secs(1),
            |In(e): In<Entity>, mut commands: Commands| {
                commands.spawn((
                    ChildOf(e),
                    Transform::from_xyz(0., 0., -1.),
                    Ring {
                        radius_to: 16.,
                        thickness_from: 3.,
                        colors: smallvec![Color::linear_rgb(4., 2., 1.)],
                        ..default()
                    },
                    Timed::new(Duration::from_millis(750)),
                ));
            },
        ),
        DebugRender::none(),
    ));

    // Entry point.
    commands.entity(level_entity).observe(
        move |trigger: Trigger<OnRemove, p2_spawn_selene::SpawningSelene>,
              mut commands: Commands| {
            commands
                .entity(level_entity)
                .insert(TutorialAlign::default());
            commands.entity(trigger.observer()).despawn();
        },
    );

    Ok(())
}
