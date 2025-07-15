use bevy::{math::VectorSpace, prelude::*};

pub trait VecExt<const COMPONENTS: usize>: VectorSpace + Sized {
    fn length_squared(self) -> f32;

    fn with_length(self, length: f32) -> Self {
        self.with_length_squared(length * length)
    }

    fn with_length_squared(self, length_squared: f32) -> Self {
        let old_length_squared = self.length_squared();
        if old_length_squared == 0. || old_length_squared == length_squared {
            self
        } else {
            self * (length_squared / old_length_squared).sqrt()
        }
    }
}

macro_rules! forward {
    ($target:ty: $count:expr) => {
        impl VecExt<$count> for $target {
            fn length_squared(self) -> f32 {
                <$target>::length_squared(self)
            }
        }
    };
}

forward!(Vec2: 2);
forward!(Vec3: 3);
