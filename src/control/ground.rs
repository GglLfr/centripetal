use crate::{
    PIXELS_PER_METER,
    control::{Jump, Movement},
    prelude::*,
};

#[derive(Component, Debug, Clone, Copy)]
#[require(GroundContacts, GroundMove, GroundJump, RigidBody::Dynamic, LockedAxes::ROTATION_LOCKED)]
pub struct GroundControl {
    /// Grounded walking speed.
    pub move_speed: f32,
    /// Horizontal force for walking while grounded.
    pub grounded_move_accel: f32,
    /// Horizontal force for changing directions while mid-air.
    pub aired_move_accel: f32,
    /// Wall-clinging climb speed.
    pub climb_speed: f32,
    /// Wall-clinging climb force for moving upwards while clinging.
    pub climb_accel: f32,
    /// Wall-clinging climb force when passively sliding down.
    pub cling_accel: f32,
    /// Wall-clinging slide force when forcefully sliding down.
    pub slide_accel: f32,
    /// Maximum jump height if the jump action is not interrupted.
    pub jump_height: f32,
    /// How much time it takes to reach [`Self::jump_max_height`].
    pub jump_duration: Duration,
    /// Grace time for jumping after falling off a platform.
    pub coyote_time: Duration,
}

impl Default for GroundControl {
    /// Defaults to a sensible all-features value, with Selene as the frame of reference.
    fn default() -> Self {
        Self {
            // While grounded, reach 4 m/s in 1/20th of a second.
            // While aired, do the same but in 1/5th of a second.
            move_speed: 4. * PIXELS_PER_METER,
            grounded_move_accel: 4. * PIXELS_PER_METER / 0.05,
            aired_move_accel: 4. * PIXELS_PER_METER / 0.2,
            // Instead of climbing, try to slide down slowly within 1/10th of a second.
            climb_speed: -2. * PIXELS_PER_METER,
            climb_accel: 9.81 * PIXELS_PER_METER / 0.1,
            cling_accel: 9.81 * PIXELS_PER_METER / 0.1,
            slide_accel: 0.,
            // Jump as high as 2.5 meters within the span of half a second.
            jump_height: 2.5 * PIXELS_PER_METER,
            jump_duration: Duration::from_millis(500),
            coyote_time: Duration::from_millis(150),
        }
    }
}

#[derive(Component, Debug, Default, Deref, DerefMut, Clone, Copy)]
pub struct GroundContacts(pub [Option<Duration>; 4]);
impl GroundContacts {
    pub const LEFT: usize = 0;
    pub const RIGHT: usize = 1;
    pub const DOWN: usize = 2;
    pub const UP: usize = 3;

    pub const DIRS: [Dir2; 4] = [Dir2::NEG_X, Dir2::X, Dir2::NEG_Y, Dir2::Y];

    pub fn is_grounded(self, now: Duration, tolerance: Duration) -> bool {
        self.0[Self::DOWN].is_some_and(|last_grounded| now.checked_sub(last_grounded).is_some_and(|offset| offset <= tolerance))
    }

    pub fn is_clinging(self, now: Duration, tolerance: Duration) -> [bool; 2] {
        [
            self.0[Self::LEFT].is_some_and(|last_cling_left| now.checked_sub(last_cling_left).is_some_and(|offset| offset <= tolerance)),
            self.0[Self::RIGHT].is_some_and(|last_cling_right| now.checked_sub(last_cling_right).is_some_and(|offset| offset <= tolerance)),
        ]
    }
}

#[derive(Component, Debug, Default, Deref, DerefMut, Clone, Copy)]
pub struct GroundMove(pub Vec2);

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct GroundJump {
    /// When the actor attempted to trigger jumping.
    pub tried: Option<Duration>,
    /// When the input system actually realized the jump.
    pub acted: Option<Duration>,
}

fn update_ground_contacts(
    time: Res<Time>,
    query: Res<SpatialQueryPipeline>,
    contacts: Query<(Entity, &Position, &Rotation, &Collider, &mut GroundContacts)>,
    layers: Query<&CollisionLayers>,
) {
    let now = time.elapsed();
    contacts.par_iter_inner().for_each(|(e, &pos, &rot, collider, contacts)| {
        let contacts = contacts.into_inner();
        let rot = rot.as_radians();
        let config = ShapeCastConfig {
            max_distance: 0.25,
            ..default()
        };

        let layer = layers.get(e).copied().unwrap_or_default();
        let filter = SpatialQueryFilter::from_mask(layer.filters);

        for (i, dir) in GroundContacts::DIRS.into_iter().enumerate() {
            query.shape_hits_callback(collider, *pos, rot, dir, &config, &filter, |data| {
                if e != data.entity
                    && (layers.get(data.entity).copied().unwrap_or_default().filters & layer.memberships) != 0
                    && (data.normal2.dot(*dir) - 1.).abs() <= 1e-4
                {
                    contacts[i] = Some(now);
                    false
                } else {
                    true
                }
            });
        }
    });
}

fn ground_move(actions: Query<(&Action<Movement>, &ActionOf<GroundControl>)>, mut ground_moves: Query<&mut GroundMove>) {
    for (action, action_of) in actions {
        let Ok(mut ground_move) = ground_moves.get_mut(action_of.entity()) else { continue };
        **ground_move = **action;
    }
}

fn ground_jump(
    time: Res<Time>,
    actions: Query<(&ActionEvents, &ActionOf<GroundControl>), With<Action<Jump>>>,
    mut ground_moves: Query<&mut GroundJump>,
) {
    let now = time.elapsed();
    for (events, action_of) in actions {
        let Ok(mut ground_jump) = ground_moves.get_mut(action_of.entity()) else { continue };
        if events.contains(ActionEvents::STARTED) {
            if ground_jump.tried.is_none() {
                ground_jump.tried = Some(now);
            }
        }

        if events.contains(ActionEvents::COMPLETED) {
            ground_jump.tried = None;
            ground_jump.acted = None;
        }
    }
}

fn evaluate_ground_control(time: Res<Time>, controls: Query<(&GroundControl, &GroundContacts, &GroundMove, &mut GroundJump, Forces)>) {
    let now = time.elapsed();
    let dt = time.delta_secs();
    controls
        .par_iter_inner()
        .for_each(|(&control, &contacts, &ground_move, mut ground_jump, mut forces)| {
            // `vel0_*`     : Current velocity.
            // `vel1_*`     : Target velocity.
            // `dv_*_target`: Total change in velocity the actor would like to make.
            // `dv_*_cap`   : Change in velocity the actor can actually make in this frame.
            // `dv_*_factor`: Multiplier to the acceleration to not overaccelerate.
            let [vel0_x, vel0_y] = forces.linear_velocity().to_array();
            let [move_x, move_y] = ground_move.clamp(Vec2::NEG_ONE, Vec2::ONE).to_array();
            let vel1_x = move_x * control.move_speed;
            let dv_x_target = vel1_x - vel0_x;

            let [cling_left, cling_right] = contacts.is_clinging(now, Duration::ZERO);
            let accel_x = match contacts.is_grounded(now, Duration::ZERO) {
                true => control.grounded_move_accel,
                false => control.aired_move_accel,
            } * if (dv_x_target > 0. && cling_right) || (dv_x_target < 0. && cling_left) { 0. } else { 1. };

            let dv_x_cap = accel_x * dt;
            let dv_x_factor = (dv_x_target.abs() / dv_x_cap).min(1.);

            let vel1_y = if cling_left || cling_right { control.climb_speed } else { vel0_y };
            let dv_y_target = vel1_y - vel0_y;
            let accel_y = match move_y.partial_cmp(&0.).unwrap_or(Equal) {
                Equal => control.cling_accel,
                Less => control.slide_accel * move_y,
                Greater => control.climb_accel * move_y,
            };

            let dv_y_cap = accel_y * dt;
            let dv_y_factor = if dv_y_cap >= 1e-4 { (dv_y_target.abs() / dv_y_cap).min(1.) } else { 0. };

            let accel = vec2(
                (accel_x * dv_x_factor).copysign(dv_x_target),
                (accel_y * dv_y_factor).copysign(dv_y_target),
            );
            forces.apply_linear_acceleration(accel);

            match (ground_jump.tried, ground_jump.acted) {
                (None, None) => {}
                (Some(tried), None) => {
                    //    y = v0ᵧt - ½gt²
                    // tₘₐₓ = v0ᵧ/g       = `control.jump_duration`
                    // yₘₐₓ = ½v0ᵧ²/g     = `control.jump_height`
                    //  v0ᵧ = √(2yₘₐₓg)
                    if contacts.is_grounded(now, control.coyote_time) {
                        ground_jump.acted = Some(now);
                        forces.linear_velocity_mut().y += (2. * control.jump_height * 9.81 * PIXELS_PER_METER).sqrt();
                    }
                }
                (.., Some(acted)) => {
                    //
                }
            };
        });
}

pub(super) fn plugin(app: &mut App) {
    app.add_input_context_to::<FixedPreUpdate, GroundControl>().add_systems(
        FixedUpdate,
        ((update_ground_contacts, ground_move, ground_jump), evaluate_ground_control)
            .chain()
            .after(PhysicsSystems::Prepare)
            .before(PhysicsSystems::StepSimulation),
    );
}
