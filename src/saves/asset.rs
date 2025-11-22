use crate::{prelude::*, saves::ReflectSave, util::IteratorExt};

#[derive(Reflect, Asset, Debug, Default)]
#[reflect(Debug, Default, FromWorld)]
pub struct SaveData {
    #[reflect(ignore)]
    resources: Vec<Box<dyn PartialReflect>>,
    #[reflect(ignore)]
    entities: EntityHashMap<Vec<Box<dyn PartialReflect>>>,
}

#[derive(Clone, Copy)]
pub struct SaveDataSerializer<'ser> {
    pub registry: &'ser TypeRegistry,
    pub data: &'ser SaveData,
}

impl Serialize for SaveDataSerializer<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        struct ResourcesSerializer<'ser> {
            registry: &'ser TypeRegistry,
            resources: &'ser [Box<dyn PartialReflect>],
        }

        impl Serialize for ResourcesSerializer<'_> {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let mut ser = serializer.serialize_seq(Some(self.resources.len()))?;
                ser.end()
            }
        }

        let mut ser = serializer.serialize_struct("SaveData", 2)?;
        ser.serialize_field("resources", &ResourcesSerializer {
            registry: self.registry,
            resources: &self.data.resources,
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
        let value_map = self.values.iter().try_map_into(HashMap::with_capacity(self.values.len()), |value| {
            let info = value
                .get_represented_type_info()
                .ok_or_else(|| ser::Error::custom("missing represented type info"))?;
            let path = info.type_path();
            let saver = self
                .registry
                .get_type_data::<ReflectSave>(info.type_id())
                .ok_or_else(|| ser::Error::custom(format!("missing `ReflectSave` for `{path}`")))?;

            Ok((path, (saver, value)))
        })?;

        let mut ser = serializer.serialize_map(Some(value_map.len()))?;
        for (path, (saver, value)) in value_map {
            ser.serialize_entry(path, value)?;
        }

        ser.end()
    }
}
