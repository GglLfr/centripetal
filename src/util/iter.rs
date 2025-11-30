pub trait IteratorExt: Iterator {
    fn try_map_into_ref<Collection: Extend<T>, T, E>(
        self,
        collection: &mut Collection,
        mut mapper: impl FnMut(Self::Item) -> Result<T, E>,
    ) -> Result<&mut Collection, E>
    where
        Self: Sized,
    {
        // TODO: When `extend_one` (#72631) is stabilized, reserve elements in `collection` with
        //       `size_hint()`'s lower bound.
        for item in self {
            let item = mapper(item)?;
            collection.extend([item]);
        }
        Ok(collection)
    }

    fn try_map_into<Collection: Extend<T>, T, E>(
        self,
        mut collection: Collection,
        mapper: impl FnMut(Self::Item) -> Result<T, E>,
    ) -> Result<Collection, E>
    where
        Self: Sized,
    {
        self.try_map_into_ref(&mut collection, mapper)?;
        Ok(collection)
    }

    fn try_map_into_default<Collection: Default + Extend<T>, T, E>(self, mapper: impl FnMut(Self::Item) -> Result<T, E>) -> Result<Collection, E>
    where Self: Sized {
        self.try_map_into(Collection::default(), mapper)
    }
}

impl<I: Iterator> IteratorExt for I {}
