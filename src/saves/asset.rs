use crate::{
    prelude::*,
    saves::{ReflectSave, ReflectedLoad, ReflectedSave},
    util::IteratorExt,
};

#[derive(Reflect, Asset, Debug, Default)]
#[reflect(Debug, Default, FromWorld)]
pub struct SaveData {
    #[reflect(ignore)]
    pub(super) resources: Vec<Box<dyn Reflect>>,
    #[reflect(ignore)]
    pub(super) entities: BTreeMap<Entity, Vec<Box<dyn Reflect>>>,
}

#[derive(Debug, Clone)]
pub struct SaveDataLoader {
    registry: TypeRegistryArc,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub enum SaveDataFormat {
    #[default]
    Guess,
    Textual,
    Binary,
}

impl AssetLoader for SaveDataLoader {
    type Asset = SaveData;
    type Settings = SaveDataFormat;
    type Error = BevyError;

    async fn load(&self, reader: &mut dyn Reader, settings: &Self::Settings, load_context: &mut LoadContext<'_>) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let binary = match settings {
            SaveDataFormat::Binary => true,
            SaveDataFormat::Textual => false,
            SaveDataFormat::Guess => load_context
                .asset_path()
                .get_full_extension()
                .map(|ext| ext.ends_with(".bin"))
                .unwrap_or(false),
        };

        let de = SaveDataDeserializer {
            registry: &*self.registry.read(),
        };

        Ok(match binary {
            false => de.deserialize(&mut ron::Deserializer::from_bytes(&bytes)?)?,
            true => de.deserialize(bincode::serde::BorrowedSerdeDecoder::from_slice(&bytes, bincode::config::standard(), ()).as_deserializer())?,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["sav.ron", "sav.bin"]
    }
}

#[derive(Clone, Copy)]
pub struct SaveDataSerializer<'ser> {
    pub registry: &'ser TypeRegistry,
    pub data: &'ser SaveData,
}

impl Serialize for SaveDataSerializer<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        struct ArchetypeSerializer<'ser> {
            registry: &'ser TypeRegistry,
            values: &'ser [Box<dyn Reflect>],
        }

        impl Serialize for ArchetypeSerializer<'_> {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let value_map = self.values.iter().try_map_into(BTreeMap::new(), |value| {
                    let info = value.reflect_type_info();
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

        struct EntitiesSerializer<'ser> {
            registry: &'ser TypeRegistry,
            entities: &'ser BTreeMap<Entity, Vec<Box<dyn Reflect>>>,
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

pub struct SaveDataDeserializer<'de> {
    pub registry: &'de TypeRegistry,
}

impl<'de> DeserializeSeed<'de> for SaveDataDeserializer<'de> {
    type Value = SaveData;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_struct("SaveData", &["resources", "entities"], self)
    }
}

impl<'de> de::Visitor<'de> for SaveDataDeserializer<'de> {
    type Value = SaveData;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "struct SaveData {{ resources, entities }}")
    }

    fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "lowercase")]
        enum Field {
            Resources,
            Entities,
        }

        struct ArchetypeDeserializer<'de> {
            registry: &'de TypeRegistry,
        }

        impl<'de> DeserializeSeed<'de> for ArchetypeDeserializer<'de> {
            type Value = Vec<Box<dyn Reflect>>;

            fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
                deserializer.deserialize_map(self)
            }
        }

        impl<'de> de::Visitor<'de> for ArchetypeDeserializer<'de> {
            type Value = Vec<Box<dyn Reflect>>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "a map of full type path and versioned data")
            }

            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut output = BTreeMap::new();
                while let Some(path) = map.next_key()? {
                    let save = self
                        .registry
                        .get_with_type_path(path)
                        .ok_or_else(|| de::Error::invalid_value(de::Unexpected::Str(path), &"no type registration found"))?
                        .data::<ReflectSave>()
                        .ok_or_else(|| de::Error::invalid_value(de::Unexpected::Str(path), &"no `ReflectSave` dat found"))?;

                    if output.insert(path, map.next_value_seed(ReflectedLoad { save })?).is_some() {
                        Err(de::Error::custom(format!("duplicated entry `{path}`")))?
                    }
                }
                Ok(output.into_values().collect())
            }
        }

        struct EntitiesDeserializer<'de> {
            registry: &'de TypeRegistry,
        }

        impl<'de> DeserializeSeed<'de> for EntitiesDeserializer<'de> {
            type Value = BTreeMap<Entity, Vec<Box<dyn Reflect>>>;

            fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
                deserializer.deserialize_map(self)
            }
        }

        impl<'de> de::Visitor<'de> for EntitiesDeserializer<'de> {
            type Value = BTreeMap<Entity, Vec<Box<dyn Reflect>>>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "a map of entity and components")
            }

            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut output = BTreeMap::new();
                while let Some((e, components)) = map.next_entry_seed(PhantomData, ArchetypeDeserializer { registry: self.registry })? {
                    if output.insert(e, components).is_some() {
                        Err(de::Error::custom(format!("duplicated entity `{e}`")))?
                    }
                }

                Ok(output)
            }
        }

        let mut resources = None;
        let mut entities = None;
        loop {
            match map.next_key()? {
                Some(Field::Resources) => {
                    if resources
                        .replace(map.next_value_seed(ArchetypeDeserializer { registry: self.registry })?)
                        .is_some()
                    {
                        Err(de::Error::duplicate_field("resources"))?
                    }
                }
                Some(Field::Entities) => {
                    if entities
                        .replace(map.next_value_seed(EntitiesDeserializer { registry: self.registry })?)
                        .is_some()
                    {
                        Err(de::Error::duplicate_field("entities"))?
                    }
                }
                None => {
                    break Ok(SaveData {
                        resources: resources.ok_or_else(|| de::Error::missing_field("resources"))?,
                        entities: entities.ok_or_else(|| de::Error::missing_field("entities"))?,
                    })
                }
            }
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_asset::<SaveData>().register_asset_reflect::<SaveData>();
}
