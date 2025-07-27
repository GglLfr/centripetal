use bevy::{
    ecs::{query::ROQueryItem, system::SystemParamItem},
    prelude::*,
    render::render_phase::{PhaseItem, RenderCommandResult, TrackedRenderPass},
};

use crate::graphics::{Fbo, FboWrappedDrawer};

#[derive(Debug, Copy, Clone, Default)]
pub struct BlitPixelizedShapes;
impl<P: PhaseItem> FboWrappedDrawer<P> for BlitPixelizedShapes {
    type Param = ();
    type ViewQuery = ();
    type ItemQuery = ();

    fn render<'w>(
        fbo: Fbo,
        item: &P,
        view: ROQueryItem<'w, Self::ViewQuery>,
        entity: Option<ROQueryItem<'w, Self::ItemQuery>>,
        param: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        RenderCommandResult::Success
    }
}
