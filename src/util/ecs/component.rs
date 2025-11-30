use crate::prelude::*;

/// Non-panicking checked methods for working with raw pointers of components. The only way to
/// construct this type data is by using the `FromType` implementation, which guarantees against
/// "malicious" implementations that might lead to unsound behavior.
#[derive(Clone, Copy)]
pub struct ReflectComponentPtr(ReflectComponentPtrFns);

#[derive(Clone, Copy)]
pub struct ReflectComponentPtrFns {
    pub into_owned_ptr: fn(Box<dyn PartialReflect>) -> Result<*mut (), Box<dyn PartialReflect>>,
    pub insert_from_ptr: unsafe fn(*mut (), &mut EntityWorldMut, RelationshipHookMode),
    pub drop_ptr: unsafe fn(*mut ()),
    pub drop_uninit_ptr: unsafe fn(*mut ()),
    pub register_component: fn(&mut World) -> ComponentId,
}

impl ReflectComponentPtr {
    pub fn component_fns(self) -> ReflectComponentPtrFns {
        self.0
    }
}

impl<T: Component + Reflect> FromType<T> for ReflectComponentPtr {
    fn from_type() -> Self {
        Self(ReflectComponentPtrFns {
            into_owned_ptr: |value| value.try_downcast::<T>().map(|value| Box::into_raw(value).cast()),
            insert_from_ptr: |ptr, entity, hook_mode| {
                entity.insert_with_relationship_hook_mode(*unsafe { Box::from_raw(ptr.cast::<T>()) }, hook_mode);
            },
            drop_ptr: |ptr| drop(unsafe { Box::from_raw(ptr.cast::<T>()) }),
            drop_uninit_ptr: |ptr| drop(unsafe { Box::from_raw(ptr.cast::<MaybeUninit<T>>()) }),
            register_component: |world| world.register_component::<T>(),
        })
    }
}

pub(super) fn plugin(app: &mut App) {
    app.register_type_data::<Transform, ReflectComponentPtr>()
        .register_type_data::<Visibility, ReflectComponentPtr>();
}
