use bevy::prelude::*;

#[derive(Debug, Copy, Clone, Component, Deref, DerefMut)]
pub struct BaseColor(pub Color);
impl Default for BaseColor {
    fn default() -> Self {
        Self(Color::WHITE)
    }
}
