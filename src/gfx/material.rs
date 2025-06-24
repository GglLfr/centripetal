use bevy::{
    ecs::system::SystemParamItem,
    prelude::*,
    render::{
        mesh::MeshVertexBufferLayoutRef,
        render_resource::{
            AsBindGroup, AsBindGroupError, BindGroupLayout, BindGroupLayoutEntry, RenderPipelineDescriptor,
            SpecializedMeshPipelineError, UnpreparedBindGroup,
        },
        renderer::RenderDevice,
    },
    sprite::{AlphaMode2d, Material2d, Material2dKey},
};

#[derive(Debug, Clone, AsBindGroup)]
pub struct GfxMaterialInner {
    #[texture(0, visibility(fragment))]
    #[sampler(1)]
    pub texture: Handle<Image>,
}

#[derive(Debug, Clone, Asset, TypePath, Deref, DerefMut)]
pub struct GfxMaterial {
    #[deref]
    pub inner: GfxMaterialInner,
    pub props: GfxMaterialProperties,
}

#[derive(Debug, Copy, Clone)]
pub struct GfxMaterialProperties {
    pub additive: bool,
}

impl AsBindGroup for GfxMaterial {
    type Data = (GfxMaterialProperties, <GfxMaterialInner as AsBindGroup>::Data);
    type Param = <GfxMaterialInner as AsBindGroup>::Param;

    fn unprepared_bind_group(
        &self,
        layout: &BindGroupLayout,
        render_device: &RenderDevice,
        param: &mut SystemParamItem<'_, '_, Self::Param>,
        force_no_bindless: bool,
    ) -> Result<UnpreparedBindGroup<Self::Data>, AsBindGroupError> {
        let bind_group = self
            .inner
            .unprepared_bind_group(layout, render_device, param, force_no_bindless)?;

        Ok(UnpreparedBindGroup {
            bindings: bind_group.bindings,
            data: (self.props, bind_group.data),
        })
    }

    fn bind_group_layout_entries(render_device: &RenderDevice, force_no_bindless: bool) -> Vec<BindGroupLayoutEntry>
    where Self: Sized {
        GfxMaterialInner::bind_group_layout_entries(render_device, force_no_bindless)
    }
}

impl Material2d for GfxMaterial {
    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if key.bind_group_data.0.additive &&
            let Some(ref mut fragment) = descriptor.fragment
        {
            //TODO set additive blending
        }

        Ok(())
    }
}
