use crate::{
    CharacterTextures, MiscTextures,
    control::{GroundControl, GroundJump, GroundMove, Jump, Movement},
    math::{GlobalTransform2d, Transform2d},
    prelude::*,
    render::{
        CameraTarget, MAIN_LAYER,
        animation::{Animation, AnimationRepeat, AnimationTag},
        painter::{Painter, PainterParam},
    },
    world::{EntityCreate, LevelSystems, MessageReaderEntityExt},
};

#[derive(Component, Debug)]
pub struct Selene;
impl Selene {
    pub const IDENT: &'static str = "selene";
    pub const IDLE: &'static str = "idle";
}

fn on_selene_spawn(mut commands: Commands, mut messages: MessageReader<EntityCreate>, textures: Res<CharacterTextures>) {
    for &EntityCreate { entity, bounds, .. } in messages.created(Selene::IDENT) {
        let sprite_center = bounds.center();
        let collider_center = vec2(sprite_center.x, bounds.min.y + 12.);

        commands.entity(entity).insert((
            Selene,
            // Hair.
            children![
                (Transform2d::from_xyz(-2.5, 4.5, -f32::EPSILON), Hair::new(3, 2., 0.1)),
                (Transform2d::from_xyz(-1.25, 3.75, -f32::EPSILON), Hair::new(5, 2., 0.1)),
                (Transform2d::from_xyz(0., 3., -f32::EPSILON), Hair::new(6, 2., 0.1)),
                (Transform2d::from_xyz(1.25, 3.75, -f32::EPSILON), Hair::new(5, 2., 0.1)),
                (Transform2d::from_xyz(2.5, 4.5, -f32::EPSILON), Hair::new(3, 2., 0.1)),
            ],
            // Transforms.
            (
                Transform2d::from_translation(sprite_center.extend(1.)),
                TransformInterpolation,
                TransformHermiteEasing,
                CameraTarget::default(),
            ),
            // Rendering.
            (Animation::from(&textures.selene), AnimationTag::new(Selene::IDLE), AnimationRepeat::Loop),
            MAIN_LAYER,
            // Physics.
            (
                SweptCcd::LINEAR,
                Collider::compound(vec![(collider_center - sprite_center, 0., Collider::rectangle(6., 24.))]),
                #[cfg(feature = "dev")]
                DebugRender::none(),
            ),
            // Inputs.
            (
                GroundControl,
                GroundMove::default(),
                GroundJump::default(),
                actions!(GroundControl[(
                    Action::<Movement>::new(),
                    Down::new(0.5),
                    Bindings::spawn(Cardinal::arrows()),
                ), (
                    Action::<Jump>::new(),
                    bindings![KeyCode::KeyZ],
                )]),
            ),
        ));
    }
}

/// Hair strand simulation with Verlet integration.
///
/// Use PBD (position-based dynamics) instead of the usual Semi-Implicit Euler Integration here as
/// the hair deals with constraints, and PBD proves the correct tool to give a nice stable result.
#[derive(Component, Debug)]
#[component(on_insert = Self::on_insert)]
#[require(Painter, Transform2d, Position, Rotation)]
pub struct Hair {
    segments: Vec<HairSegment>,
    segment_length: f32,
    damping: f32,
    last_anchor: Vec2,
}

impl Hair {
    pub fn new(length: usize, segment_length: f32, damping: f32) -> Self {
        Self {
            segments: Vec::with_capacity(length),
            segment_length,
            damping,
            last_anchor: Vec2::NAN,
        }
    }

    fn on_insert(
        mut world: DeferredWorld,
        HookContext {
            entity,
            relationship_hook_mode,
            ..
        }: HookContext,
    ) {
        if matches!(relationship_hook_mode, RelationshipHookMode::RunIfNotLinked | RelationshipHookMode::Skip) {
            return
        }

        let (mut entities, mut commands) = world.entities_and_commands();
        let Some(mut this) = entities.get_mut(entity).ok().and_then(|e| e.into_mut::<Self>()) else { return };

        let cap = this.segments.capacity();
        this.segments.resize_with(cap, || HairSegment {
            entity: commands.spawn((ChildOf(entity), Transform2d::default(), TransformInterpolation)).id(),
            position: Vec2::NAN,
            last_position: Vec2::NAN,
        });
    }
}

#[derive(Debug, Clone, Copy)]
struct HairSegment {
    entity: Entity,
    position: Vec2,
    last_position: Vec2,
}

fn update_hair_segments(time: Res<Time>, gravity: Res<Gravity>, hairs: Query<(&mut Hair, &Position)>) {
    let dt = time.delta_secs();
    let dt2 = dt * dt;
    let g = gravity.0;

    hairs.par_iter_inner().for_each(|(hair, &pos)| {
        let hair = hair.into_inner();
        if hair.last_anchor.is_nan() {
            hair.last_anchor = *pos;
        }

        for (i, seg) in hair.segments.iter_mut().enumerate() {
            if seg.position.is_nan() {
                let init_pos = *pos + vec2(0., (i + 1) as f32 * -hair.segment_length);
                seg.position = init_pos;
                seg.last_position = init_pos;
            } else {
                // x_[t + Δt] = 2x_[t] - x_[t - Δt] + aΔt^2
                // Assume gravity is the only acceleration applied to each segment.
                let implicit_vel = (seg.position - seg.last_position) * (1. - hair.damping);

                let new_position = seg.position + implicit_vel + g * dt2;
                seg.last_position = mem::replace(&mut seg.position, new_position);
            }
        }

        for _ in 0..4 {
            let Some(first) = hair.segments.first_mut() else { continue };
            {
                let dir = first.position - *pos;
                let dir_len2 = dir.length_squared();
                if dir_len2 > 1e-5 {
                    let dir_len = dir_len2.sqrt();
                    let err = (dir_len - hair.segment_length) / dir_len;
                    first.position -= dir * err;
                } else {
                    first.position = *pos + vec2(0., -hair.segment_length);
                }
            }

            let Some(end) = hair.segments.len().checked_sub(1) else { continue };
            for i in 0..end {
                let delta = hair.segments[i + 1].position - hair.segments[i].position;
                let delta_len2 = delta.length_squared();
                if delta_len2 <= 1e-5 {
                    continue
                }

                let delta_len = delta_len2.sqrt();
                let correction = delta * 0.5 * (delta_len - hair.segment_length) / delta_len;

                if i == 0 {
                    hair.segments[i + 1].position -= correction * 2.;
                } else {
                    hair.segments[i].position += correction;
                    hair.segments[i + 1].position -= correction;
                }
            }
        }

        hair.last_anchor = *pos;
    });
}

fn writeback_hair_transforms(hairs: Query<(&Hair, &GlobalTransform, &Position, &Rotation)>, mut query: Query<&mut Transform>) {
    for (hair, &global_trns, &pos, &rot) in hairs {
        for &seg in &hair.segments {
            let Ok(mut trns) = query.get_mut(seg.entity) else { continue };

            let parent_transform = global_trns.compute_transform();
            let parent_pos = pos.extend(parent_transform.translation.z);
            let parent_rot = Quat::from(rot);
            let parent_scale = parent_transform.scale;

            let parent_transform = GlobalTransform::from(Transform::from_translation(parent_pos).with_rotation(parent_rot).with_scale(parent_scale));
            let new_transform = GlobalTransform::from(Transform::from_translation(seg.position.extend(parent_pos.z * parent_scale.z)))
                .reparented_to(&parent_transform);

            *trns = new_transform;
        }
    }
}

fn draw_hair(
    param: PainterParam,
    misc: Res<MiscTextures>,
    hairs: Query<(&Painter, &Hair, &GlobalTransform2d)>,
    hair_transforms: Query<&GlobalTransform2d>,
) {
    for (painter, hair, &hair_trns) in hairs {
        let mut ctx = param.ctx(painter);
        ctx.layer = hair_trns.z;
        ctx.color = Srgba::hex("70A3C4").unwrap().into();

        let count = hair.segments.len();
        let mut prev_pos = hair_trns.affine.translation;
        for (i, &trns) in hair_transforms.iter_many(hair.segments.iter().map(|seg| seg.entity)).enumerate() {
            ctx.line(
                &misc.white,
                prev_pos,
                (count - i + 1) as f32 / count as f32 * 2.1,
                trns.affine.translation,
                (count - i) as f32 / count as f32 * 1.1,
            );

            prev_pos = trns.affine.translation;
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, on_selene_spawn.in_set(LevelSystems::SpawnEntities))
        .add_systems(
            FixedPostUpdate,
            (update_hair_segments, writeback_hair_transforms)
                .chain()
                .in_set(PhysicsSystems::Writeback),
        )
        .add_systems(PostUpdate, draw_hair.after(TransformSystems::Propagate));
}
