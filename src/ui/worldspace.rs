use bevy::{prelude::*, ui::Val::*};

use crate::logic::CameraQuery;

#[derive(Debug, Copy, Clone, Component)]
pub struct WorldspaceUi {
    pub target: Entity,
    pub offset: Vec2,
}

pub fn update_worldspace_ui(
    camera: CameraQuery<(&Camera, &Transform)>,
    mut nodes: Query<(&WorldspaceUi, &mut Node, &ComputedNode, &ComputedNodeTarget)>,
    transforms: Query<(&Transform, Option<&ChildOf>)>,
) {
    let (camera, &camera_trns) = camera.into_inner();
    let camera_trns = GlobalTransform::from(camera_trns);

    nodes
        .par_iter_mut()
        .for_each(|(&ui, mut node, computed, &target)| {
            let Ok((trns, mut child_of)) = transforms.get(ui.target) else {
                return;
            };

            let mut trns = *trns;
            while let Some(has_child_of) = child_of
                && let Ok((&parent_trns, parent_child_of)) = transforms.get(has_child_of.parent())
            {
                trns = parent_trns * trns;
                child_of = parent_child_of;
            }

            let Some(ndc) =
                camera.world_to_ndc(&camera_trns, trns.translation + ui.offset.extend(0.))
            else {
                return;
            };

            let res = target.logical_size();
            let factor = target.scale_factor();

            let xy = res * (ndc.xy() + 1.) * 0.5;
            let center = 0.5 / factor;

            node.left = Px(xy.x - computed.size.x * center);
            node.top = Px(res.y - xy.y - computed.size.y * center);
        });
}
