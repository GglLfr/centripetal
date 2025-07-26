use std::time::Duration;

use bevy::prelude::*;
use bevy_vector_shapes::shapes::{
    DiscComponent, FillType, ShapeFill, ShapeMaterial, ThicknessType,
};
use smallvec::{SmallVec, smallvec};

use crate::{
    logic::Timed,
    math::{FloatTransformer, Interp},
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
    pub radius: f32,
    pub thickness: f32,
    pub colors: SmallVec<[Color; 2]>,
    pub radius_interp: Interp<f32>,
    pub thickness_interp: Interp<f32>,
    pub color_interp: Interp<f32>,
    pub alpha_interp: Interp<f32>,
}

impl Default for Ring {
    fn default() -> Self {
        Self {
            radius: 24.,
            thickness: 2.,
            colors: smallvec![Color::WHITE],
            radius_interp: Interp::Identity,
            thickness_interp: Interp::Reverse,
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
            disc.radius = ring.radius_interp.apply(f) * ring.radius;

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
                ring.thickness_interp.apply(f) * ring.thickness,
                ThicknessType::World,
            );
        });
}
