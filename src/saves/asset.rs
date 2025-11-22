use crate::{
    prelude::*,
    saves::{ReflectSave, ReflectedSave},
    util::IteratorExt,
};

#[derive(Reflect, Asset, Debug, Default)]
#[reflect(Debug, Default, FromWorld)]
pub struct SaveData {
    #[reflect(ignore)]
    resources: Vec<Box<dyn PartialReflect>>,
    #[reflect(ignore)]
    entities: BTreeMap<Entity, Vec<Box<dyn PartialReflect>>>,
}

#[derive(Clone, Copy)]
pub struct SaveDataSerializer<'ser> {
    pub registry: &'ser TypeRegistry,
    pub data: &'ser SaveData,
}

impl Serialize for SaveDataSerializer<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        struct EntitiesSerializer<'ser> {
            registry: &'ser TypeRegistry,
            entities: &'ser BTreeMap<Entity, Vec<Box<dyn PartialReflect>>>,
        }

        impl Serialize for EntitiesSerializer<'_> {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let mut ser = serializer.serialize_map(Some(self.entities.len()))?;
                for (e, components) in self.entities {
                    ser.serialize_entry(e, &ArchetypeSerializer {
                        registry: self.registry,
                        values: components.as_slice(),
                    })?;
                }

                ser.end()
            }
        }

        let mut ser = serializer.serialize_struct("SaveData", 2)?;
        ser.serialize_field("resources", &ArchetypeSerializer {
            registry: self.registry,
            values: &self.data.resources,
        })?;
        ser.serialize_field("entities", &EntitiesSerializer {
            registry: self.registry,
            entities: &self.data.entities,
        })?;
        ser.end()
    }
}

#[derive(Clone, Copy)]
struct ArchetypeSerializer<'ser> {
    registry: &'ser TypeRegistry,
    values: &'ser [Box<dyn PartialReflect>],
}

impl Serialize for ArchetypeSerializer<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let value_map = self.values.iter().try_map_into(BTreeMap::new(), |value| {
            let info = value
                .get_represented_type_info()
                .ok_or_else(|| ser::Error::custom("missing represented type info"))?;
            let path = info.type_path();
            let save = self
                .registry
                .get_type_data::<ReflectSave>(info.type_id())
                .ok_or_else(|| ser::Error::custom(format!("missing `ReflectSave` for `{path}`")))?;

            Ok((path, (save, value.as_ref())))
        })?;

        let mut ser = serializer.serialize_map(Some(value_map.len()))?;
        for (path, (save, value)) in value_map {
            ser.serialize_entry(path, &ReflectedSave { save, value })?;
        }

        ser.end()
    }
}
