use std::time::Duration;

use bevy::prelude::*;
use bevy_vector_shapes::shapes::{
    DiscComponent, FillType, ShapeFill, ShapeMaterial, ThicknessType,
};
use smallvec::{SmallVec, smallvec};

use crate::{
    Observed,
    logic::Timed,
    math::{FloatTransformExt as _, FloatTransformer, Interp},
};

#[derive(Debug, Clone, Component)]
#[require(
    Transform,
    Visibility,
    DiscComponent,
    ShapeFill,
    ShapeMaterial,
    Timed::new(Duration::from_secs(1))
)]
pub struct Ring {
    pub radius_from: f32,
    pub radius_to: f32,
    pub thickness_from: f32,
    pub thickness_to: f32,
    pub colors: SmallVec<[Color; 2]>,
    pub radius_interp: Interp<f32>,
    pub thickness_interp: Interp<f32>,
    pub color_interp: Interp<f32>,
    pub alpha_interp: Interp<f32>,
}

impl Ring {
    pub fn bundle(self) -> impl Bundle {
        (self, Observed::by(Timed::despawn_on_finished))
    }
}

impl Default for Ring {
    fn default() -> Self {
        Self {
            radius_from: 0.,
            radius_to: 24.,
            thickness_from: 2.,
            thickness_to: 0.,
            colors: smallvec![Color::WHITE],
            radius_interp: Interp::Identity,
            thickness_interp: Interp::Identity,
            color_interp: Interp::Identity,
            alpha_interp: Interp::Reverse,
        }
    }
}

pub fn update_ring(mut rings: Query<(&Ring, &mut DiscComponent, &mut ShapeFill, &Timed)>) {
    rings
        .par_iter_mut()
        .for_each(|(ring, mut disc, mut fill, timed)| {
            let f = timed.frac();
            disc.radius = ring
                .radius_interp
                .apply(f)
                .re_map(ring.radius_from, ring.radius_to);

            let color = ring.color_interp.apply(f) * ring.colors.len() as f32;
            let color_i = color as usize;
            let color_f = color.fract();

            fill.color = match (ring.colors.get(color_i), ring.colors.get(color_i + 1)) {
                (None, ..) => Color::WHITE,
                (Some(from), None) => *from,
                (Some(from), Some(to)) => from.mix(&to, color_f),
            }
            .with_alpha(ring.alpha_interp.apply(f));

            fill.ty = FillType::Stroke(
                ring.thickness_interp
                    .apply(f)
                    .re_map(ring.thickness_from, ring.thickness_to),
                ThicknessType::World,
            );
        });
}
