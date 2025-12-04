use crate::{
    GameState, MapAssetIds, ReflectMapAssetIds,
    math::Transform2d,
    prelude::*,
    render::{MAIN_LAYER, MainCamera, atlas::AtlasRegion},
    saves::{ReflectSave, Save},
    util::ecs::ReflectComponentPtr,
};

pub const TILE_PIXEL_SIZE: f32 = 8.;
pub const TILEMAP_CHUNK_SIZE: u32 = 64;

#[derive(Reflect, Save, Component, MapAssetIds, MapEntities, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[component(immutable, on_insert = on_tile_insert, on_replace = on_tile_replace)]
#[version(0 = Self)]
#[reflect(Save, Component, ComponentPtr, MapAssetIds, MapEntities, Debug, Clone, PartialEq, Hash)]
pub struct Tile {
    #[entities]
    pub tilemap: Entity,
    pub pos: UVec2,
    #[assets]
    pub region: AssetId<AtlasRegion>,
}

impl Tile {
    pub fn new(tilemap: Entity, pos: UVec2, region: impl Into<AssetId<AtlasRegion>>) -> Self {
        Self {
            tilemap,
            pos,
            region: region.into(),
        }
    }

    pub fn index(self, dimension: UVec2) -> usize {
        self.pos.y as usize * dimension.x as usize + self.pos.x as usize
    }
}

fn on_tile_insert(
    mut world: DeferredWorld,
    HookContext {
        entity,
        relationship_hook_mode,
        ..
    }: HookContext,
) {
    if matches!(relationship_hook_mode, RelationshipHookMode::RunIfNotLinked | RelationshipHookMode::Skip) {
        return
    }

    let &tile = world.get::<Tile>(entity).unwrap();
    let tilemap = world.get_mut::<Tilemap>(tile.tilemap).expect("Missing `Tilemap` component").into_inner();
    tilemap.change_chunk(tile.pos);

    let dim = tilemap.dimension;
    if let Some(old_tile) = tilemap
        .tiles
        .get_mut(tile.index(dim))
        .unwrap_or_else(|| panic!("`Tile` {} out of bounds of {}", tile.pos, dim))
        .replace(entity)
    {
        world.commands().entity(old_tile).try_despawn();
    }
}

fn on_tile_replace(
    mut world: DeferredWorld,
    HookContext {
        entity,
        relationship_hook_mode,
        ..
    }: HookContext,
) {
    if matches!(relationship_hook_mode, RelationshipHookMode::RunIfNotLinked | RelationshipHookMode::Skip) {
        return
    }

    let &tile = world.get::<Tile>(entity).unwrap();
    let mut tilemap = world.get_mut::<Tilemap>(tile.tilemap).expect("Missing `Tilemap` component");

    let dim = tilemap.dimension;
    let current_tile = tilemap
        .bypass_change_detection()
        .tiles
        .get_mut(tile.index(dim))
        .unwrap_or_else(|| panic!("`Tile` {} out of bounds of {}", tile.pos, dim));

    if current_tile.is_some_and(|curr| curr == entity) {
        *current_tile = None;
        tilemap.change_chunk(tile.pos);
    }
}

#[derive(Reflect, Save, Component, MapEntities, Debug, Clone)]
#[require(TilemapChunks, TilemapParallax, Transform2d, Visibility)]
#[component(on_despawn = on_tilemap_despawn)]
#[version(0 = Self)]
#[reflect(Save, Component, ComponentPtr, MapEntities, Debug, Clone)]
pub struct Tilemap {
    dimension: UVec2,
    #[entities]
    tiles: Vec<Option<Entity>>,
    #[reflect(ignore)]
    changed_chunks: HashSet<UVec2>,
}

impl Serialize for Tilemap {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        struct TilesSerializer<'a>(UVec2, &'a [Option<Entity>]);
        impl Serialize for TilesSerializer<'_> {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let &Self(dimension, tiles) = self;
                let width = dimension.x as usize;

                let mut ser = serializer.serialize_seq(Some(tiles.iter().flatten().count()))?;
                for (i, tile) in tiles.iter().enumerate() {
                    let Some(tile) = tile else { continue };
                    let x = (i % width) as u32;
                    let y = (i / width) as u32;
                    ser.serialize_element(&(x, y, tile))?;
                }
                ser.end()
            }
        }

        let mut ser = serializer.serialize_struct("Tilemap", 2)?;
        ser.serialize_field("dimension", &self.dimension)?;
        ser.serialize_field("tiles", &TilesSerializer(self.dimension, &self.tiles))?;
        ser.end()
    }
}

impl<'de> Deserialize<'de> for Tilemap {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TilemapVisitor;
        impl<'de> de::Visitor<'de> for TilemapVisitor {
            type Value = Tilemap;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "a `Tilemap`")
            }

            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut dimension = None;
                let mut tiles = None;

                while let Some(field) = map.next_key()? {
                    match field {
                        "dimension" => {
                            if dimension.replace(map.next_value()?).is_some() {
                                Err(de::Error::duplicate_field("dimension"))?
                            }
                        }
                        "tiles" => {
                            if tiles.replace(map.next_value::<Vec<(u32, u32, Entity)>>()?).is_some() {
                                Err(de::Error::duplicate_field("tiles"))?
                            }
                        }
                        unknown => Err(de::Error::unknown_field(unknown, &["dimension", "tiles"]))?,
                    }
                }

                let mut tilemap = Tilemap::new(dimension.ok_or_else(|| de::Error::missing_field("dimension"))?);
                let width = tilemap.dimension.x as usize;
                for (x, y, tile) in tiles.ok_or_else(|| de::Error::missing_field("tiles"))? {
                    tilemap.tiles[y as usize * width + x as usize] = Some(tile);
                    tilemap.changed_chunks.insert(uvec2(x, y) / TILEMAP_CHUNK_SIZE);
                }

                Ok(tilemap)
            }
        }

        deserializer.deserialize_struct("Tilemap", &["dimension", "tiles"], TilemapVisitor)
    }
}

impl Tilemap {
    pub fn new(dimension: UVec2) -> Self {
        Self {
            dimension,
            tiles: vec![None; dimension.x as usize * dimension.y as usize],
            changed_chunks: default(),
        }
    }

    pub fn dimension(&self) -> UVec2 {
        self.dimension
    }

    pub fn clear(&mut self, commands: &mut Commands) {
        for tile in &mut self.tiles {
            if let Some(entity) = tile.take() {
                commands.entity(entity).try_despawn();
            }
        }
    }

    pub fn resize(&mut self, new_dimension: UVec2, commands: &mut Commands) {
        let old_width = mem::replace(&mut self.dimension, new_dimension).x;
        let old_tiles = mem::replace(&mut self.tiles, vec![None; new_dimension.x as usize * new_dimension.y as usize]);

        for (i, tile) in old_tiles.into_iter().enumerate().filter_map(|(i, tile)| Some((i, tile?))) {
            let x = i % old_width as usize;
            let y = i / old_width as usize;

            let index = y * new_dimension.y as usize + x;
            if let Some(dst) = self.tiles.get_mut(index) {
                *dst = Some(tile);
            } else {
                commands.entity(tile).try_despawn();
            }
        }
    }

    pub fn change_chunk(&mut self, pos: UVec2) {
        self.changed_chunks.insert(pos / TILEMAP_CHUNK_SIZE);
    }

    pub fn chunk_size_at(&self, pos: UVec2) -> UVec2 {
        let start = (pos * TILEMAP_CHUNK_SIZE).min(self.dimension);
        let end = ((pos + 1) * TILEMAP_CHUNK_SIZE).min(self.dimension);
        end - start
    }

    pub fn iter_tiles(&self) -> impl Iterator<Item = (UVec2, Entity)> {
        let width = self.dimension.x;
        let height = self.dimension.y;

        (0..height).flat_map(move |y| (0..width).filter_map(move |x| Some((uvec2(x, y), self.tiles[y as usize * width as usize + x as usize]?))))
    }

    pub fn iter_changed_chunks(&self) -> impl Iterator<Item = UVec2> + ExactSizeIterator {
        self.changed_chunks.iter().copied()
    }

    pub fn iter_chunk(&self, chunk: UVec2) -> impl Iterator<Item = (UVec2, Option<Entity>)> {
        let [x, y] = chunk.to_array();
        let width = self.dimension.x;
        let height = self.dimension.y;

        let x_range = x * TILEMAP_CHUNK_SIZE..((x + 1) * TILEMAP_CHUNK_SIZE).min(width);
        let y_range = y * TILEMAP_CHUNK_SIZE..((y + 1) * TILEMAP_CHUNK_SIZE).min(height);

        y_range.flat_map(move |y| {
            x_range
                .clone()
                .map(move |x| (uvec2(x, y), self.tiles[y as usize * width as usize + x as usize]))
        })
    }
}

fn on_tilemap_despawn(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    let (entities, mut commands) = world.entities_and_commands();
    for &tile in entities.get(entity).unwrap().get::<Tilemap>().unwrap().tiles.iter().flatten() {
        commands.entity(tile).try_despawn();
    }
}

fn clear_tilemap_changed_chunks(tilemaps: Query<&mut Tilemap>) {
    for mut tilemap in tilemaps {
        tilemap.bypass_change_detection().changed_chunks.clear();
    }
}

#[derive(Reflect, Component, Debug, Default, Clone)]
#[reflect(Component, Debug, Default, FromWorld, Clone)]
pub struct TilemapChunks {
    last_dimension: UVec2,
    chunk_entities: HashMap<UVec2, Entity>,
}

#[derive(Reflect, Component, Debug, Default, Clone, Copy)]
#[require(Transform2d, Visibility)]
#[reflect(Component, Debug, Default, FromWorld, Clone)]
pub struct TilemapChunk {
    pub size: UVec2,
}

fn update_tilemap_chunks(
    mut commands: Commands,
    tilemaps: Query<(Entity, &Tilemap, &mut TilemapChunks), Changed<Tilemap>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    regions: Res<Assets<AtlasRegion>>,
    tiles: Query<&Tile>,
) {
    let regions = regions.into_inner();
    let mesh_handle_allocator = &meshes.get_handle_provider();
    let material_handle_allocator = &materials.get_handle_provider();

    for (mesh_id, mesh, material_id, material, chunk_bundle) in ComputeTaskPool::get()
        .scope(|scope| {
            for (tilemap_entity, tilemap, mut chunks) in tilemaps {
                if chunks
                    .reborrow()
                    .map_unchanged(|chunk| &mut chunk.last_dimension)
                    .set_if_neq(tilemap.dimension)
                {
                    for (.., e) in chunks.chunk_entities.drain() {
                        commands.entity(e).despawn();
                    }
                }

                for chunk_pos in tilemap.iter_changed_chunks() {
                    let chunk_entity = {
                        let e = commands
                            .spawn((
                                ChildOf(tilemap_entity),
                                TilemapChunk {
                                    size: tilemap.chunk_size_at(chunk_pos),
                                },
                                Transform2d {
                                    translation: (chunk_pos.as_vec2() * TILEMAP_CHUNK_SIZE as f32).extend(0.),
                                    ..default()
                                },
                            ))
                            .id();

                        if let Some(old_chunk_entity) = chunks.chunk_entities.insert(chunk_pos, e) {
                            commands.entity(old_chunk_entity).despawn();
                        }
                        e
                    };

                    scope.spawn(async move {
                        let mut for_image = HashMap::new();
                        for (pos, tile) in tilemap.iter_chunk(chunk_pos) {
                            let Some(&tile) = tile.and_then(|e| tiles.get(e).ok()) else { continue };
                            let Some(region) = regions.get(tile.region) else { continue };

                            let [bx, by] = ((pos % TILEMAP_CHUNK_SIZE).as_vec2() * TILE_PIXEL_SIZE).to_array();
                            let [tx, ty] = [bx + TILE_PIXEL_SIZE, by + TILE_PIXEL_SIZE];
                            let (positions, uvs, indices): &mut (Vec<_>, Vec<_>, Vec<_>) = &mut for_image.entry(&region.page.texture).or_default();

                            let i = positions.len() as u16;
                            indices.extend([i, i + 1, i + 2, i + 2, i + 3, i]);

                            positions.extend([[bx, by, 0.], [tx, by, 0.], [tx, ty, 0.], [bx, ty, 0.]]);
                            uvs.extend(region.uv_corners().map(|uv| uv.to_array()));
                        }

                        for_image
                            .into_iter()
                            .map(move |(image, (positions, uvs, indices))| {
                                let mesh_handle = mesh_handle_allocator.reserve_handle().typed();
                                let mesh_handle_id = mesh_handle.id();

                                let material_handle = material_handle_allocator.reserve_handle().typed();
                                let material_handle_id = material_handle.id();

                                (
                                    mesh_handle_id,
                                    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
                                        .with_inserted_indices(Indices::U16(indices))
                                        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, VertexAttributeValues::Float32x3(positions))
                                        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, VertexAttributeValues::Float32x2(uvs)),
                                    material_handle_id,
                                    ColorMaterial {
                                        color: Color::WHITE,
                                        alpha_mode: AlphaMode2d::Blend,
                                        uv_transform: Affine2::IDENTITY,
                                        texture: Some(image.clone()),
                                    },
                                    (
                                        ChildOf(chunk_entity),
                                        Aabb::from_min_max(Vec3::ZERO, Vec2::splat(TILEMAP_CHUNK_SIZE as f32 * TILE_PIXEL_SIZE as f32).extend(0.)),
                                        Mesh2d(mesh_handle),
                                        MeshMaterial2d(material_handle),
                                        MAIN_LAYER,
                                    ),
                                )
                            })
                            .collect::<Box<_>>()
                    });
                }
            }
        })
        .into_iter()
        .flatten()
    {
        meshes.insert(mesh_id, mesh).expect("ID is reserved using `reserve_handle()`");
        materials.insert(material_id, material).expect("ID is reserved using `reserve_handle()`");
        commands.spawn(chunk_bundle);
    }
}

#[derive(Reflect, Save, Component, Debug, Clone, Copy, Serialize, Deserialize)]
#[require(Transform2d)]
#[version(0 = Self)]
#[reflect(Save, Component, ComponentPtr, Debug, Default, FromWorld, Clone)]
pub struct TilemapParallax {
    pub factor: Vec2,
    pub scale: bool,
}

impl Default for TilemapParallax {
    fn default() -> Self {
        Self {
            factor: Vec2::ZERO,
            scale: false,
        }
    }
}

fn update_tilemap_parallax(tilemaps: Query<(&mut Transform2d, &Tilemap, &TilemapParallax), Without<MainCamera>>, camera: Single<&MainCamera>) {
    let camera_pos = camera.pos;
    for (mut trns, tilemap, &parallax) in tilemaps {
        let center = tilemap.dimension.as_vec2() / 2.;
        let dst = camera_pos - center;
        trns.set_if_neq(Transform2d {
            translation: (dst * parallax.factor).round().extend(trns.translation.z),
            rotation: Rot2::IDENTITY,
            scale: if parallax.scale { Vec2::ONE - parallax.factor } else { Vec2::ONE },
        });
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TilemapSystems {
    ClearChangedChunks,
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        (
            (
                update_tilemap_chunks,
                clear_tilemap_changed_chunks.in_set(TilemapSystems::ClearChangedChunks),
            )
                .chain()
                // TODO use computed state for `InGame`.
                .run_if(in_state(GameState::InGame { paused: false })),
            update_tilemap_parallax.before(TransformSystems::Propagate),
        ),
    );
}
