use crate::{math::Transform2d, prelude::*};

/// Hair strand simulation with Verlet integration.
///
/// Use PBD (position-based dynamics) instead of the usual Semi-Implicit Euler Integration here as
/// the hair deals with constraints, and PBD proves the correct tool to give a nice stable result.
#[derive(Component, Debug)]
#[component(on_insert = Self::on_insert)]
#[require(Transform, TransformInterpolation)]
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

    pub fn last_anchor(&self) -> Vec2 {
        self.last_anchor
    }

    pub fn iter_segments(&self) -> impl Iterator<Item = HairSegment> + ExactSizeIterator + DoubleEndedIterator {
        self.segments.iter().copied()
    }

    pub fn iter_strands(&self) -> impl Iterator<Item = Entity> + ExactSizeIterator + DoubleEndedIterator {
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
            let trns = Transform::from_translation(Vec3::NAN);
            seg.entity = commands.spawn((trns, Transform2d::from(trns), TransformInterpolation)).id();
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HairSegment {
    pub entity: Entity,
    pub position: Vec2,
    pub last_position: Vec2,
    pub length: f32,
}

fn update_hair_segments(
    time: Res<Time>,
    gravity: Res<Gravity>,
    hairs: Query<(&mut Hair, &Transform, Option<&ChildOf>)>,
    parent_query: Query<(&Position, &Rotation, &GlobalTransform)>,
) {
    let dt = time.delta_secs();
    let dt2 = dt * dt;
    let g = gravity.0;

    hairs.par_iter_inner().for_each(|(hair, &hair_trns, hair_child_of)| {
        let pos = &if let Some(child_of) = hair_child_of
            && let Ok((&parent_pos, &parent_rot, &parent_trns)) = parent_query.get(child_of.parent())
        {
            let parent_trns = parent_trns.compute_transform();
            (Transform {
                translation: parent_pos.extend(parent_trns.translation.z),
                rotation: Quat::from(parent_rot),
                scale: parent_trns.scale,
            } * hair_trns)
                .translation
                .truncate()
        } else {
            hair_trns.translation.truncate()
        };

        let hair = hair.into_inner();
        if hair.last_anchor.is_nan() {
            hair.last_anchor = *pos;
        }

        let mut accum_pos = *pos;
        for seg in &mut hair.segments {
            if seg.position.is_nan() {
                accum_pos += vec2(0., -seg.length);
                seg.position = accum_pos;
                seg.last_position = accum_pos;
            } else {
                // x_[t + Δt] = 2x_[t] - x_[t - Δt] + aΔt^2
                // Assume gravity is the only acceleration applied to each segment.
                let implicit_vel = (seg.position - seg.last_position) * (1. - hair.damping);

                let new_position = seg.position + implicit_vel + g * dt2;
                seg.last_position = mem::replace(&mut seg.position, new_position);
            }
        }

        const ITER_COUNT: usize = 8;
        for iter in 1..=ITER_COUNT {
            let Some(first) = hair.segments.first_mut() else { continue };
            {
                let dir = first.position - *pos;
                let dir_len2 = dir.length_squared();
                if dir_len2 > 1e-4 {
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
                if delta_len2 <= 1e-4 {
                    continue
                }

                let delta_len = delta_len2.sqrt();
                let correction = delta * 0.5 * (delta_len - hair.segments[i + 1].length) / delta_len;

                // On the `ITER_COUNT`'th iteration, convergence should have been reached.
                // In that case, *enforce* the distance constraint instead of distributing it evenly, so that
                // segments don't gradually move further from the root segment.
                if i == 0 || iter == ITER_COUNT {
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

fn writeback_hair_transforms(hairs: Query<(&Hair, &GlobalTransform)>, mut query: Query<&mut Transform>) {
    for (hair, &global_trns) in hairs {
        for &seg in &hair.segments {
            let Ok(mut trns) = query.get_mut(seg.entity) else { continue };

            let parent_trns = global_trns.compute_transform();
            let translation = seg.position.extend(parent_trns.translation.z * parent_trns.scale.z);
            *trns = Transform::from_translation(translation);
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
