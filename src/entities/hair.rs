use crate::{math::Transform2d, prelude::*};

/// Hair strand simulation with Verlet integration.
///
/// Use PBD (position-based dynamics) instead of the usual Semi-Implicit Euler Integration here as
/// the hair deals with constraints, and PBD proves the correct tool to give a nice stable result.
#[derive(Component, Debug)]
#[component(on_insert = Self::on_insert)]
#[require(Transform, Position, Rotation)]
pub struct Hair {
    segments: Vec<HairSegment>,
    damping: f32,
    last_anchor: Vec2,
}

impl Hair {
    pub fn new(segment_lengths: impl IntoIterator<Item = f32>, damping: f32) -> Self {
        Self {
            segments: segment_lengths
                .into_iter()
                .map(|length| HairSegment {
                    entity: Entity::PLACEHOLDER,
                    position: Vec2::NAN,
                    last_position: Vec2::NAN,
                    length,
                })
                .collect(),
            damping,
            last_anchor: Vec2::NAN,
        }
    }

    pub fn iter_strands(&self) -> impl Iterator<Item = Entity> + ExactSizeIterator {
        self.segments.iter().map(|seg| seg.entity)
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

        for seg in &mut this.segments {
            seg.entity = commands.spawn((ChildOf(entity), Transform2d::default(), TransformInterpolation)).id();
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct HairSegment {
    entity: Entity,
    position: Vec2,
    last_position: Vec2,
    length: f32,
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
            // TODO accumulate position
            if seg.position.is_nan() {
                seg.position = *pos;
                seg.last_position = *pos;
            } else {
                // x_[t + Δt] = 2x_[t] - x_[t - Δt] + aΔt^2
                // Assume gravity is the only acceleration applied to each segment.
                let implicit_vel = (seg.position - seg.last_position) * (1. - hair.damping);

                let new_position = seg.position + implicit_vel + g * dt2;
                seg.last_position = mem::replace(&mut seg.position, new_position);
            }
        }

        // TODO Probably use XPBD to avoid iterating *this* much...
        for _ in 0..15 {
            let Some(first) = hair.segments.first_mut() else { continue };
            {
                let dir = first.position - *pos;
                let dir_len2 = dir.length_squared();
                if dir_len2 > 1e-5 {
                    let dir_len = dir_len2.sqrt();
                    let err = (dir_len - first.length) / dir_len;
                    first.position -= dir * err;
                } else {
                    first.position = *pos + vec2(0., -first.length);
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
                let correction = delta * 0.5 * (delta_len - hair.segments[i + 1].length) / delta_len;

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

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        FixedPostUpdate,
        (update_hair_segments, writeback_hair_transforms)
            .chain()
            .in_set(PhysicsSystems::Writeback),
    );
}
