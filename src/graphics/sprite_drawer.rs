use std::ops::Deref;

use bevy::{asset::weak_handle, prelude::*};
use vec_belt::VecBelt;

#[derive(Component)]
#[require(Transform, Visibility)]
pub struct SpriteDrawer {
    queued: VecBelt<Draw>,
}

impl Default for SpriteDrawer {
    fn default() -> Self {
        Self {
            queued: VecBelt::new(1),
        }
    }
}

impl SpriteDrawer {
    pub fn draw(&self, conf: Draw) {
        self.queued.append([conf]);
    }

    pub fn draw_at(&self, pos: impl Into<Vec3>, rot: impl Into<Rot2>, sprite: Sprite) {
        self.draw(Draw {
            pos: pos.into(),
            rot: rot.into(),
            sprite,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct Draw {
    pub pos: Vec3,
    pub rot: Rot2,
    pub sprite: Sprite,
}

pub fn flush_drawer_to_children(
    mut commands: Commands,
    mut drawers: Query<(Entity, &mut SpriteDrawer, Option<&Children>)>,
    mut sprite_items: Query<(&mut Sprite, &mut Transform)>,
) {
    for (drawer_entity, mut drawer, sprites) in &mut drawers {
        let sprites = sprites.map(Children::deref).unwrap_or(&[]);
        let mut sprites = sprite_items.iter_many_mut(sprites);

        let mut drawer_entity = commands.entity(drawer_entity);
        drawer.queued.clear(|slice| {
            for draw in slice {
                let transform = Transform {
                    translation: draw.pos,
                    rotation: Quat::from_axis_angle(Vec3::Z, draw.rot.as_radians()),
                    scale: Vec3::ONE,
                };

                if let Some((mut sprite, mut trns)) = sprites.fetch_next() {
                    *sprite = draw.sprite;
                    *trns = transform;
                } else {
                    drawer_entity.with_child((draw.sprite, transform));
                }
            }
        });

        while let Some((mut leftover_sprite, ..)) = sprites.fetch_next() {
            leftover_sprite.image = const { weak_handle!("68b49790-e39e-497c-ab88-af97bac6e172") };
        }
    }
}
