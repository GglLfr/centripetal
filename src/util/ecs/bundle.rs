use crate::prelude::*;

pub struct RawBundle {}

impl RawBundle {
    pub fn from_reflect(world: &mut World, components: impl IntoIterator<Item = Box<dyn Reflect>>) -> Result<Self> {
        let registry = world.resource::<AppTypeRegistry>().clone();
        for component in components {
            //
        }

        todo!()
    }
}

pub(super) fn plugin(app: &mut App) {}
