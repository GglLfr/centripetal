use crate::{
    GRAVITY, PIXELS_PER_METER,
    control::{Jump, Movement},
    prelude::*,
};

#[derive(Component, Debug, Default, Clone)]
#[require(GroundContacts, RigidBody::Dynamic, Mass = Self::LIGHT, AngularInertia(f32::MAX), NoAutoMass, NoAutoAngularInertia, LockedAxes::ROTATION_LOCKED)]
pub struct GroundControl {
    pub contact_shape: Collider,
    pub contact_distance: f32,
}

#[derive(Component, Debug, Default, Deref, DerefMut, Clone, Copy)]
pub struct GroundContacts(pub [Option<GroundContact>; 4]);
impl GroundContacts {
    pub const LEFT: usize = 0;
    pub const RIGHT: usize = 1;
    pub const DOWN: usize = 2;
    pub const UP: usize = 3;

    pub const DIRS: [Dir2; 4] = [Dir2::NEG_X, Dir2::X, Dir2::NEG_Y, Dir2::Y];

    pub fn is_touching(self, index: usize, now: Duration, tolerance: Duration) -> bool {
        self.0[index]
            .map(|contact| now.checked_sub(contact.since).is_some_and(|dt| dt <= tolerance))
            .unwrap_or(false)
    }

    pub fn is_grounded(self, now: Duration, tolerance: Duration) -> bool {
        self.is_touching(Self::DOWN, now, tolerance)
    }

    pub fn is_grounded_and_velocity(self, now: Duration, tolerance: Duration) -> Option<Vec2> {
        self.0[Self::DOWN].and_then(|contact| {
            now.checked_sub(contact.since)
                .is_some_and(|dt| dt <= tolerance)
                .then_some(contact.linear_velocity.unwrap_or(Vec2::ZERO))
        })
    }

    pub fn is_head_bumping(self, now: Duration, tolerance: Duration) -> bool {
        self.is_touching(Self::UP, now, tolerance)
    }

    pub fn is_clinging(self, now: Duration, tolerance: Duration) -> [bool; 2] {
        [
            self.is_touching(Self::LEFT, now, tolerance),
            self.is_touching(Self::RIGHT, now, tolerance),
        ]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GroundContact {
    pub since: Duration,
    pub linear_velocity: Option<Vec2>,
}

fn update_ground_contacts(
    time: Res<Time>,
    query: Res<SpatialQueryPipeline>,
    contacts: Query<(Entity, &Position, &Rotation, &GroundControl, &mut GroundContacts)>,
    layers: Query<&CollisionLayers>,
    velocities: Query<&LinearVelocity>,
) {
    let now = time.elapsed();
    contacts.par_iter_inner().for_each(|(e, &pos, &rot, control, contacts)| {
        let contacts = contacts.into_inner();
        let rot = rot.as_radians();
        let config = ShapeCastConfig {
            max_distance: control.contact_distance,
            target_distance: control.contact_distance,
            ignore_origin_penetration: true,
            ..default()
        };

        let layer = layers.get(e).copied().unwrap_or_default();
        let filter = SpatialQueryFilter::from_mask(layer.filters);

        for (i, dir) in GroundContacts::DIRS.into_iter().enumerate() {
            query.shape_hits_callback(&control.contact_shape, *pos, rot, dir, &config, &filter, |data| {
                if e != data.entity
                    && (layers.get(data.entity).copied().unwrap_or_default().filters & layer.memberships) != 0
                    && -data.normal1.dot(*dir) >= 0.5
                {
                    contacts[i] = Some(GroundContact {
                        since: now,
                        linear_velocity: velocities.get(data.entity).ok().map(|v| **v),
                    });
                    false
                } else {
                    true
                }
            });
        }
    })
}

#[derive(Component, Debug, Clone, Copy)]
#[require(GroundMoveState)]
pub struct GroundMove {
    /// Grounded walking speed.
    pub move_speed: f32,
    /// Horizontal force for walking while grounded.
    pub grounded_move_accel: f32,
    /// Horizontal force for changing directions while mid-air.
    pub aired_move_accel: f32,
}

impl Default for GroundMove {
    fn default() -> Self {
        Self {
            // Reach 4 m/s in 1/20th of a second.
            move_speed: 4. * PIXELS_PER_METER,
            grounded_move_accel: 4. * PIXELS_PER_METER / (1. / 20.),
            aired_move_accel: 4. * PIXELS_PER_METER / (1. / 20.),
        }
    }
}

#[derive(Component, Debug, Default, Clone, Copy, Deref, DerefMut)]
struct GroundMoveState(Vec2);

impl GroundControl {
    pub const LIGHT: Mass = Mass(50.);
}

fn ground_move(actions: Query<(&Action<Movement>, &ActionOf<GroundControl>)>, mut ground_moves: Query<&mut GroundMoveState>) {
    for (action, action_of) in actions {
        let Ok(mut ground_move) = ground_moves.get_mut(action_of.entity()) else { continue };
        **ground_move = **action;
    }
}

#[derive(Component, Debug, Clone, Copy)]
#[require(GroundJumpState)]
pub struct GroundJump {
    /// Maximum jump height if the jump action is not interrupted.
    pub jump_height: f32,
    /// Grace time for jumping when attempted to do so before grounded.
    pub buffer_time: Duration,
    /// Grace time for jumping after falling off a platform.
    pub coyote_time: Duration,
}

impl Default for GroundJump {
    fn default() -> Self {
        Self {
            // Jump as high as 2.5 meters.
            jump_height: 2.5 * PIXELS_PER_METER,
            buffer_time: Duration::from_millis(100),
            coyote_time: Duration::from_millis(150),
        }
    }
}

#[derive(Component, Debug, Default, Clone, Copy)]
struct GroundJumpState {
    tried: Option<Duration>,
    acted: bool,
    time: Option<f32>,
}

fn ground_jump(
    time: Res<Time>,
    actions: Query<(&ActionEvents, &ActionOf<GroundControl>), With<Action<Jump>>>,
    mut jump_states: Query<&mut GroundJumpState>,
) {
    let now = time.elapsed();
    for (events, action_of) in actions {
        let Ok(mut ground_jump) = jump_states.get_mut(action_of.entity()) else { continue };
        if events.contains(ActionEvents::STARTED) {
            if ground_jump.tried.is_none() {
                ground_jump.tried = Some(now);
            }
        }

        if events.contains(ActionEvents::COMPLETED) {
            ground_jump.tried = None;
            ground_jump.acted = false;
        }
    }
}

fn evaluate_ground(
    time: Res<Time>,
    states: Query<(
        &mut GroundContacts,
        Option<(&GroundMove, &GroundMoveState)>,
        Option<(&GroundJump, &mut GroundJumpState)>,
        Forces,
    )>,
) {
    let now = time.elapsed();
    let dt = time.delta_secs();
    states.par_iter_inner().for_each(|(mut contacts, movement, jump, mut forces)| {
        if let Some((&param, &state)) = movement {
            // `vel0`     : Current velocity.
            // `vel1`     : Target velocity.
            // `dv_target`: Total change in velocity the actor would like to make.
            // `dv_cap`   : Change in velocity the actor can actually make in this frame.
            // `dv_factor`: Multiplier to the acceleration to not overaccelerate.
            let vel0_x = forces.linear_velocity().x;
            let vel1_x = state.x.clamp(-1., 1.) * param.move_speed;
            let dv_x_target = vel1_x - vel0_x;

            let [cling_left, cling_right] = contacts.is_clinging(now, Duration::ZERO);
            let accel_x = match contacts.is_grounded(now, Duration::ZERO) {
                true => param.grounded_move_accel,
                false => param.aired_move_accel,
            } * if (dv_x_target > 0. && cling_right) || (dv_x_target < 0. && cling_left) { 0. } else { 1. };

            let dv_x_cap = accel_x * dt;
            let dv_x_factor = (dv_x_target.abs() / dv_x_cap).min(1.);

            forces.apply_linear_acceleration(vec2((accel_x * dv_x_factor).copysign(dv_x_target), 0.));
        }

        if let Some((&param, mut state)) = jump {
            // Apply an upwards velocity of sqrt(2gh), as stated in high school physics class.
            // If the actor stops jumping before reaching maximum height, cancel the impulse.
            match (state.tried, state.acted) {
                (None, false) => {
                    if let Some(commited) = state.time.take() {
                        let total = (2. * param.jump_height * GRAVITY).sqrt();
                        if commited < total / GRAVITY {
                            let leftover = total - GRAVITY * commited;
                            forces.linear_velocity_mut().y -= leftover;
                        }
                    }
                }
                (Some(tried), false) => {
                    if let Some(ground_velocity) = contacts.is_grounded_and_velocity((tried + param.buffer_time).min(now), param.coyote_time) {
                        info!("Jump!");
                        // Disable coyote time on jump.
                        contacts[GroundContacts::DOWN] = None;
                        state.acted = true;
                        state.time = Some(0.);

                        forces.linear_velocity_mut().y = ground_velocity.y + (2. * param.jump_height * GRAVITY).sqrt();
                    }
                }
                (.., true) => {
                    if contacts.is_head_bumping(now, Duration::ZERO) {
                        state.tried = None;
                        state.acted = false;
                        state.time = None;
                    } else {
                        *state.time.get_or_insert(0.) += dt;
                    }
                }
            };
        }
    });
}

pub(super) fn plugin(app: &mut App) {
    app.add_input_context_to::<FixedPreUpdate, GroundControl>()
        .add_systems(FixedUpdate, (update_ground_contacts, (ground_move, ground_jump), evaluate_ground).chain());
}
