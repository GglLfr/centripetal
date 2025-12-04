use crate::{control::Movement, prelude::*};

#[derive(Component, Debug, Default)]
#[require(GroundContacts, RigidBody::Dynamic, LockedAxes::ROTATION_LOCKED)]
pub struct GroundController {}

#[derive(Component, Debug, Default, Deref, DerefMut)]
pub struct GroundContacts(pub [bool; 4]);
impl GroundContacts {
    pub const LEFT: usize = 0;
    pub const RIGHT: usize = 1;
    pub const DOWN: usize = 2;
    pub const UP: usize = 3;

    pub const DIRS: [Dir2; 4] = [Dir2::NEG_X, Dir2::X, Dir2::NEG_Y, Dir2::Y];

    pub const fn is_grounded(self) -> bool {
        self.0[Self::DOWN]
    }
}

fn update_ground_contacts(
    query: Res<SpatialQueryPipeline>,
    contacts: Query<(Entity, &Position, &Rotation, &Collider, &mut GroundContacts)>,
    layers: Query<&CollisionLayers>,
) {
    contacts.par_iter_inner().for_each(|(e, &pos, &rot, collider, contacts)| {
        let contacts = contacts.into_inner();
        let rot = rot.as_radians();
        let config = ShapeCastConfig {
            max_distance: 1e-5,
            ..default()
        };

        let layer = layers.get(e).copied().unwrap_or_default();
        let filter = SpatialQueryFilter::from_mask(layer.filters);

        for (i, dir) in GroundContacts::DIRS.into_iter().enumerate() {
            contacts[i] = false;
            query.shape_hits_callback(collider, *pos, rot, dir, &config, &filter, |data| {
                if e != data.entity
                    && (layers.get(data.entity).copied().unwrap_or_default().filters & layer.memberships) != 0
                    && (data.normal2.dot(*dir) - 1.).abs() <= 1e-5
                {
                    contacts[i] = true;
                    false
                } else {
                    true
                }
            });
        }
    });
}

fn on_ground_move(movement: On<Fire<Movement>>, mut control: Query<&mut GroundController>) {
    //
}

pub(super) fn plugin(app: &mut App) {
    app.add_input_context::<GroundController>()
        .add_systems(Update, update_ground_contacts)
        .add_observer(on_ground_move);
}
