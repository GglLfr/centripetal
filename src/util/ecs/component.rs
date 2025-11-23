use crate::prelude::*;

/// Non-panicking checked methods for working with raw pointers of components. The only way to
/// construct this type data is by using the `FromType` implementation, which maximally guarantees
/// against "malicious" implementations that might lead to unsound behavior.
#[derive(Clone, Copy)]
pub struct ReflectComponentPtr {
    into_owned_ptr: fn(Box<dyn PartialReflect>) -> Result<*mut (), Box<dyn PartialReflect>>,
    register_component: fn(&mut World) -> ComponentId,
}

impl<T: Component> FromType<T> for ReflectComponentPtr {
    fn from_type() -> Self {
        Self {
            into_owned_ptr: |value| value.try_downcast::<T>().map(|value| Box::into_raw(value).cast()),
            register_component: |world| world.register_component::<T>(),
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    //
}
