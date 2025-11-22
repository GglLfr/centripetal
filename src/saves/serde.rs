use crate::prelude::*;

/// Do not change this, ever!
pub type SaveVersion = u32;

pub trait Save: Serialize + for<'de> Deserialize<'de> + Reflectable {
    fn saver() -> SaverWithInput<Self>;

    fn loader() -> LoaderWithOutput<Self>;
}

pub trait SaveSpec<const VERSION: SaveVersion>: Save + Sized {
    type Repr: Serialize + for<'de> Deserialize<'de>;
}

pub struct Saver<T: Save, const VERSION: SaveVersion>
where T: SaveSpec<VERSION>
{
    _marker: PhantomData<fn(<T as SaveSpec<VERSION>>::Repr)>,
}

impl<T: Save, const VERSION: SaveVersion> Saver<T, VERSION>
where T: SaveSpec<VERSION>
{
    pub fn new() -> Self {
        Self { _marker: PhantomData }
    }

    pub fn finish(self) -> SaverWithInput<<T as SaveSpec<VERSION>>::Repr> {
        SaverWithInput {
            version: VERSION,
            _marker: PhantomData,
        }
    }
}

pub struct SaverWithInput<Repr: Serialize> {
    version: SaveVersion,
    _marker: PhantomData<fn(&Repr)>,
}

pub struct Loader<T: Save, const VERSION: SaveVersion>
where T: SaveSpec<VERSION>
{
    accumulated_versions: HashSet<SaveVersion>,
    loader: Box<dyn Fn(SaveVersion, &mut dyn erased_serde::Deserializer) -> erased_serde::Result<T::Repr> + 'static + Send + Sync>,
}

impl<T: Save, const VERSION: SaveVersion> Loader<T, VERSION>
where T: SaveSpec<VERSION>
{
    pub fn new() -> Self {
        Self {
            accumulated_versions: [VERSION].into(),
            loader: Box::new(|version, deserializer| {
                if version == VERSION {
                    <T as SaveSpec<VERSION>>::Repr::deserialize(deserializer)
                } else {
                    Err(de::Error::custom(format!("unknown version {version} for {}", type_name::<T>())))
                }
            }),
        }
    }

    pub fn next<const TO: SaveVersion>(
        mut self,
        mapper: impl Fn(<T as SaveSpec<VERSION>>::Repr) -> <T as SaveSpec<TO>>::Repr + 'static + Send + Sync,
    ) -> Loader<T, TO>
    where
        T: SaveSpec<TO>,
    {
        if !self.accumulated_versions.insert(TO) {
            panic!("Duplicated version {VERSION} for {}", type_name::<T>())
        }

        let prev_loader = self.loader;
        Loader {
            accumulated_versions: self.accumulated_versions,
            loader: Box::new(move |version, deserializer| {
                if version == TO {
                    <T as SaveSpec<TO>>::Repr::deserialize(deserializer)
                } else {
                    let prev_repr = prev_loader(version, deserializer)?;
                    let new_repr = mapper(prev_repr);
                    Ok(new_repr)
                }
            }),
        }
    }

    pub fn finish(self) -> LoaderWithOutput<<T as SaveSpec<VERSION>>::Repr> {
        LoaderWithOutput { loader: self.loader }
    }
}

pub struct LoaderWithOutput<Repr: for<'de> Deserialize<'de>> {
    loader: Box<LoaderFunction<Repr>>,
}

type LoaderFunction<T> = dyn Fn(SaveVersion, &mut dyn erased_serde::Deserializer) -> erased_serde::Result<T> + 'static + Send + Sync;

#[derive(Clone)]
pub struct ReflectSave {
    saver: Arc<dyn Fn(&dyn PartialReflect, &mut dyn erased_serde::SerializeTuple) -> erased_serde::Result<()> + 'static + Send + Sync>,
    loader: Arc<dyn Fn(&mut dyn erased_serde::Deserializer) -> erased_serde::Result<Box<dyn PartialReflect>> + 'static + Send + Sync>,
}

impl<T: Save> FromType<T> for ReflectSave {
    fn from_type() -> Self {
        let saver_spec = T::saver();
        let loader_spec = T::loader();

        Self {
            saver: Arc::new(move |repr, ser| {
                let repr = repr
                    .try_downcast_ref::<T>()
                    .ok_or_else(|| <erased_serde::Error as ser::Error>::custom(format!("value expected to be `{}`", type_name::<T>())))?;

                ser.erased_serialize_element(&saver_spec.version).map_err(erased_serde::Error::from)?;
                ser.erased_serialize_element(repr).map_err(erased_serde::Error::from)?;
                Ok(())
            }),
            loader: Arc::new(move |deserializer: &mut dyn erased_serde::Deserializer| {
                struct Visit<'a, T>(&'a LoaderFunction<T>);
                impl<'de, T: Reflect> de::Visitor<'de> for Visit<'_, T> {
                    type Value = Box<dyn PartialReflect>;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        write!(formatter, "a tuple of save version and raw data")
                    }

                    fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                        struct LoadVersioned<'a, T>(SaveVersion, &'a LoaderFunction<T>);
                        impl<'de, T: Reflect> DeserializeSeed<'de> for LoadVersioned<'_, T> {
                            type Value = Box<dyn PartialReflect>;

                            fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
                                let Self(version, loader) = self;
                                loader(version, &mut <dyn erased_serde::Deserializer>::erase(deserializer))
                                    .map(|value| Box::new(value) as Box<dyn PartialReflect>)
                                    .map_err(|e| e.as_de_typed())
                            }
                        }

                        let version = seq.next_element::<SaveVersion>()?.ok_or(de::Error::invalid_length(0, &"2"))?;
                        Ok(seq
                            .next_element_seed(LoadVersioned(version, self.0))?
                            .ok_or(de::Error::invalid_length(1, &"2"))?)
                    }
                }

                deserializer.deserialize_tuple(2, Visit(&loader_spec.loader))
            }),
        }
    }
}

#[derive(Clone, Copy)]
pub struct ReflectedSave<'de> {
    pub save: &'de ReflectSave,
    pub value: &'de dyn PartialReflect,
}

impl Serialize for ReflectedSave<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut ser = erased_serde::erase::Serializer::<S>::Tuple(serializer.serialize_tuple(2)?);
        (self.save.saver)(self.value, &mut ser).map_err(|e| e.as_ser_typed())?;

        let erased_serde::erase::Serializer::Tuple(ser) = ser else { unreachable!() };
        ser.end()
    }
}
