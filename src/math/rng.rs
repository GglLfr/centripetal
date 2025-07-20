use std::iter::FusedIterator;

use bevy::prelude::*;
use fastrand::Rng;

pub struct RandLenVectors<'a> {
    pub rng: &'a mut Rng,
    pub count: usize,
    pub angle_from: f32,
    pub angle_to: f32,
    pub len_from: f32,
    pub len_to: f32,
}

impl Iterator for RandLenVectors<'_> {
    type Item = (Rot2, Vec2);

    fn next(&mut self) -> Option<Self::Item> {
        self.count = self.count.checked_sub(1)?;

        let (sin, cos) = self
            .angle_from
            .lerp(self.angle_to, self.rng.f32())
            .sin_cos();
        let len = self.len_from.lerp(self.len_to, self.rng.f32());

        Some((Rot2 { sin, cos }, vec2(cos * len, sin * len)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.count, Some(self.count))
    }
}

impl ExactSizeIterator for RandLenVectors<'_> {
    fn len(&self) -> usize {
        self.count
    }
}

impl FusedIterator for RandLenVectors<'_> {}

pub trait RngExt {
    fn as_rng(&mut self) -> &mut Rng;

    fn f32_within(&mut self, min: f32, max: f32) -> f32 {
        min + self.as_rng().f32() * (max - min)
    }

    fn f64_within(&mut self, min: f64, max: f64) -> f64 {
        min + self.as_rng().f64() * (max - min)
    }

    fn len_vectors(
        &mut self,
        count: usize,
        angle_from: f32,
        angle_to: f32,
        len_from: f32,
        len_to: f32,
    ) -> RandLenVectors<'_> {
        RandLenVectors {
            rng: self.as_rng(),
            count,
            angle_from,
            angle_to,
            len_from,
            len_to,
        }
    }
}

impl RngExt for Rng {
    fn as_rng(&mut self) -> &mut Rng {
        self
    }
}
