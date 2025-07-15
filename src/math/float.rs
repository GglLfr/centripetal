use std::cmp::Ordering;

use num_traits::Float;

pub trait FloatTransformer<T: Float> {
    fn apply(&self, value: T) -> T {
        self.apply_within(value, T::zero(), T::one())
    }

    fn apply_within(&self, value: T, min: T, max: T) -> T;
}

#[derive(Debug, Copy, Clone)]
pub struct Threshold<T: Float> {
    pub from: T,
    pub to: T,
}

impl<T: Float> FloatTransformer<T> for Threshold<T> {
    fn apply_within(&self, value: T, _min: T, _max: T) -> T {
        (value.clamp(self.from, self.to) - self.from) / (self.to - self.from)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Slope<T: Float> {
    pub mid: T,
}

impl<T: Float> FloatTransformer<T> for Slope<T> {
    fn apply_within(&self, value: T, min: T, max: T) -> T {
        match value.partial_cmp(&self.mid).unwrap_or(Ordering::Equal) {
            Ordering::Less => min + (value - min) / (self.mid - min) * (max - min),
            Ordering::Equal => max,
            Ordering::Greater => max - (value - self.mid) / (max - self.mid) * (max - min),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct PowIn {
    pub exponent: u32,
}

impl<T: Float> FloatTransformer<T> for PowIn {
    fn apply_within(&self, value: T, min: T, max: T) -> T {
        min + ((value - min) / (max - min)).powi(self.exponent as i32) * (max - min)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct PowOut {
    pub exponent: u32,
}

impl<T: Float> FloatTransformer<T> for PowOut {
    fn apply_within(&self, value: T, min: T, max: T) -> T {
        let bounds = max - min;
        let scl = (value - min) / bounds;
        let scl = (scl - T::one()).powi(self.exponent as i32) + [T::zero(), T::one()][(self.exponent % 2) as usize];
        min + scl * bounds
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Pow {
    pub exponent: u32,
}

impl<T: Float> FloatTransformer<T> for Pow {
    fn apply_within(&self, value: T, min: T, max: T) -> T {
        let exponent = self.exponent;
        let bounds = max - min;
        let scl = (value - min) / bounds;

        let half = T::one() / (T::one() + T::one());
        min + match scl.partial_cmp(&half).unwrap_or(Ordering::Equal) {
            Ordering::Less => PowIn { exponent }.apply_within(scl, T::zero(), half),
            Ordering::Equal => half,
            Ordering::Greater => PowOut { exponent }.apply_within(scl, half, T::one()),
        } * bounds
    }
}

pub trait FloatExt: Float {
    fn threshold(self, from: Self, to: Self) -> Self {
        Threshold { from, to }.apply(self)
    }

    fn slope(self, mid: Self) -> Self {
        Slope { mid }.apply(self)
    }

    fn pow(self, exponent: u32) -> Self {
        self.pow_within(exponent, Self::zero(), Self::one())
    }

    fn pow_within(self, exponent: u32, min: Self, max: Self) -> Self {
        Pow { exponent }.apply_within(self, min, max)
    }

    fn pow_in(self, exponent: u32) -> Self {
        self.pow_in_within(exponent, Self::zero(), Self::one())
    }

    fn pow_in_within(self, exponent: u32, min: Self, max: Self) -> Self {
        PowIn { exponent }.apply_within(self, min, max)
    }

    fn pow_out(self, exponent: u32) -> Self {
        self.pow_out_within(exponent, Self::zero(), Self::one())
    }

    fn pow_out_within(self, exponent: u32, min: Self, max: Self) -> Self {
        PowOut { exponent }.apply_within(self, min, max)
    }
}

impl<T: Float> FloatExt for T {}
