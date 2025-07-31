use std::cmp::Ordering;

use num_traits::Float;

pub trait FloatTransformer<T: Float> {
    fn apply(&self, value: T) -> T {
        self.apply_within(value, T::zero(), T::one())
    }

    fn apply_within(&self, value: T, min: T, max: T) -> T;
}

impl<T: Float> FloatTransformer<T> for Box<dyn FloatTransformer<T>> {
    fn apply_within(&self, value: T, min: T, max: T) -> T {
        (**self).apply_within(value, min, max)
    }
}

impl<T: Float> FloatTransformer<T> for &dyn FloatTransformer<T> {
    fn apply_within(&self, value: T, min: T, max: T) -> T {
        (*self).apply_within(value, min, max)
    }
}

impl<T: Float, F: Fn(T) -> T> FloatTransformer<T> for F {
    fn apply_within(&self, value: T, min: T, max: T) -> T {
        self((value - min) / (max - min))
    }
}

impl<T: Float> FloatTransformer<T> for () {
    fn apply_within(&self, value: T, _min: T, _max: T) -> T {
        value
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Remap<T: Float> {
    pub min: T,
    pub max: T,
}

impl<T: Float> FloatTransformer<T> for Remap<T> {
    fn apply_within(&self, value: T, min: T, max: T) -> T {
        self.min + (value - min) / (max - min) * (self.max - self.min)
    }
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

        let mut scl = (scl - T::one()).powi(self.exponent as i32);
        if self.exponent % 2 == 0 {
            scl = T::one() - scl;
        } else {
            scl = scl + T::one();
        }

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

        // What a fancy way to write `0.5` in an infallible way.
        let half = T::one() / (T::one() + T::one());
        min + match scl.partial_cmp(&half).unwrap_or(Ordering::Equal) {
            Ordering::Less => PowIn { exponent }.apply_within(scl, T::zero(), half),
            Ordering::Equal => half,
            Ordering::Greater => PowOut { exponent }.apply_within(scl, half, T::one()),
        } * bounds
    }
}

#[derive(Debug, Clone)]
pub enum Interp<T: Float> {
    Identity,
    Reverse,
    Slope { mid: T },
    PowIn { exponent: u32 },
    PowOut { exponent: u32 },
    Pow { exponent: u32 },
    Chain(Vec<Self>),
}

impl<T: Float> FloatTransformer<T> for Interp<T> {
    fn apply_within(&self, value: T, min: T, max: T) -> T {
        match *self {
            Self::Identity => value,
            Self::Reverse => max - value + min,
            Self::Slope { mid } => Slope { mid }.apply_within(value, min, max),
            Self::PowIn { exponent } => PowIn { exponent }.apply_within(value, min, max),
            Self::PowOut { exponent } => PowOut { exponent }.apply_within(value, min, max),
            Self::Pow { exponent } => Pow { exponent }.apply_within(value, min, max),
            Self::Chain(ref chain) => chain
                .iter()
                .fold(value, |value, interp| interp.apply_within(value, min, max)),
        }
    }
}

pub trait FloatTransformExt: Float {
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

    fn re_map(self, min: Self, max: Self) -> Self {
        Remap { min, max }.apply(self)
    }

    fn min_mag(self, min: Self) -> Self {
        self.copysign(self.abs().min(min))
    }

    fn max_mag(self, max: Self) -> Self {
        self.copysign(self.abs().max(max))
    }
}

impl<T: Float> FloatTransformExt for T {}
