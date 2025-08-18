use crate::prelude::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Reflect, Actionlike, Serialize, Deserialize)]
pub struct LaunchAction;

#[derive(Debug, Copy, Clone, Default, Component, Deref, DerefMut)]
#[require(ActionState<LaunchAction>, LaunchIdle)]
pub struct LaunchTarget(pub Option<Entity>);

#[derive(Debug, Clone, Default, Component, Deref, DerefMut)]
pub struct LaunchDurations(pub Vec<Duration>);

#[derive(Debug, Copy, Clone, Default, Component, Deref, DerefMut)]
pub struct LaunchCooldown(pub Duration);

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
pub struct LaunchIdle {
    pub last_attempted: Duration,
}

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
pub struct LaunchCharging {
    pub index: usize,
    pub time: Duration,
}

#[derive(Debug, Copy, Clone, Default, Component)]
#[component(storage = "SparseSet")]
pub struct LaunchFinished {
    pub index: usize,
}

#[derive(Debug, Clone, Event)]
pub struct TryLaunch {
    by: Entity,
    at: Entity,
    index: usize,
    stopped: bool,
    current_hit: Option<RayHitData>,
    hits: Vec<RayHitData>,
}

impl TryLaunch {
    pub fn by(&self) -> Entity {
        self.by
    }

    pub fn at(&self) -> Entity {
        self.at
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn stop(&mut self) {
        self.stopped = true;
    }

    /// # Panics
    ///
    /// Panics if called on observers observing [`Self::by`], which is always triggered initially.
    pub fn hit_data(&self) -> RayHitData {
        self.current_hit.unwrap()
    }
}

impl EntityCommand<Result> for TryLaunch {
    fn apply(mut self, mut entity: EntityWorldMut) -> Result {
        let by = entity.id();
        self.by = by; // Sanity assignment.
        let hits = std::mem::take(&mut self.hits);

        let stopped_by = entity.world_scope(|world| {
            world.trigger_targets_ref(&mut self, by);
            if self.stopped {
                return by;
            }

            for hit in hits {
                self.current_hit = Some(hit);
                world.trigger_targets_ref(&mut self, hit.entity);

                if self.stopped {
                    return hit.entity;
                }
            }

            Entity::PLACEHOLDER
        });

        if stopped_by == Entity::PLACEHOLDER {
            entity.trigger(Launched {
                at: self.at,
                index: self.index,
            });
        } else {
            entity.trigger(LaunchFailed { stopped_by });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Event)]
pub struct LaunchFailed {
    pub stopped_by: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct LaunchCancelled;

#[derive(Debug, Clone, Event)]
pub struct Launched {
    pub at: Entity,
    pub index: usize,
}

pub fn update_launch_idle(
    mut commands: Commands,
    time: Res<Time>,
    idle: Query<(Entity, &ActionState<LaunchAction>, &LaunchTarget, &LaunchIdle, Option<&LaunchCooldown>)>,
) {
    let now = time.elapsed();
    for (e, state, &target, &idle, cooldown) in &idle {
        if !state.just_pressed(&LaunchAction) {
            continue
        }

        let cooldown = *cooldown.copied().unwrap_or_default();
        if target.is_some() && now - idle.last_attempted >= cooldown {
            commands.entity(e).remove::<LaunchIdle>().insert(LaunchCharging::default());
        }
    }
}

pub fn update_launch_charging(
    mut commands: Commands,
    time: Res<Time>,
    mut charging: Query<(Entity, &ActionState<LaunchAction>, &LaunchDurations, &mut LaunchCharging)>,
) {
    let now = time.elapsed();
    let delta = time.delta();

    for (e, action, durations, mut charging) in &mut charging {
        let Some(&duration) = durations.get(charging.index) else { continue };
        if !action.pressed(&LaunchAction) {
            if charging.index > 0 {
                commands
                    .entity(e)
                    .remove::<LaunchCharging>()
                    .insert(LaunchFinished { index: charging.index - 1 });
            } else {
                commands
                    .entity(e)
                    .remove::<LaunchCharging>()
                    .insert(LaunchIdle { last_attempted: now })
                    .trigger(LaunchCancelled);
            }

            continue
        }

        charging.time += delta;
        if let Some(remainder) = charging.time.checked_sub(duration) {
            if durations.get(charging.index + 1).is_some() {
                charging.index += 1;
                charging.time = remainder;
            } else {
                commands
                    .entity(e)
                    .remove::<LaunchCharging>()
                    .insert(LaunchFinished { index: charging.index });
            }
        }
    }
}

pub fn trigger_launch_charging(
    time: Res<Time>,
    mut commands: Commands,
    pipeline: Res<SpatialQueryPipeline>,
    charging: Query<(Entity, &Position, &LaunchTarget, &LaunchFinished)>,
    layers: Query<&CollisionLayers>,
    targets: Query<&Position>,
) {
    let now = time.elapsed();
    for (e, &pos, &target, &finished) in &charging {
        if let Some(target) = *target
            && let Ok(&target_pos) = targets.get(target)
            && let Ok((dir, len)) = Dir2::new_and_length(*target_pos - *pos)
        {
            let layer = layers.get(e).copied().unwrap_or_default();
            let mut hits = Vec::new();
            pipeline.ray_hits_callback(*pos, dir, len, true, &SpatialQueryFilter::from_mask(layer.filters), |hit| {
                let other_layer = layers.get(hit.entity).copied().unwrap_or_default();
                if (other_layer.filters & layer.memberships) != 0 {
                    hits.push(hit);
                }

                true
            });

            hits.sort_unstable_by_key(|data| FloatOrd(data.distance));
            commands
                .entity(e)
                .insert(LaunchIdle { last_attempted: now })
                .remove::<LaunchFinished>()
                .queue_handled(
                    TryLaunch {
                        by: e,
                        at: target,
                        index: finished.index,
                        stopped: false,
                        current_hit: None,
                        hits,
                    },
                    warn,
                );
        }
    }
}
