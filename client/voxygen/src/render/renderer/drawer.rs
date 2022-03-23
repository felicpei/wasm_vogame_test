use super::{
    super::{
        buffer::Buffer,
        instances::Instances,
        model::{DynamicModel, Model, SubModel},
        pipelines::{
            blit, bloom, clouds, debug, figure, fluid, lod_terrain, particle, shadow, skybox,
            sprite, terrain, ui, ColLights, GlobalsBindGroup, ShadowTexturesBindGroup,
        },
    },
    Renderer, ShadowMap, ShadowMapRenderer,
};
use core::{num::NonZeroU32, ops::Range};
use std::sync::Arc;
use vek::Aabr;

// Currently available pipelines
enum Pipelines<'frame> {
    Interface(&'frame super::InterfacePipelines),
    All(&'frame super::Pipelines),
    // Should never be in this state for now but we need this to accound for super::State::Nothing
    None,
}

impl<'frame> Pipelines<'frame> {
    fn ui(&self) -> Option<&ui::UiPipeline> {
        match self {
            Pipelines::Interface(pipelines) => Some(&pipelines.ui),
            Pipelines::All(pipelines) => Some(&pipelines.ui),
            Pipelines::None => None,
        }
    }

    fn blit(&self) -> Option<&blit::BlitPipeline> {
        match self {
            Pipelines::Interface(pipelines) => Some(&pipelines.blit),
            Pipelines::All(pipelines) => Some(&pipelines.blit),
            Pipelines::None => None,
        }
    }

    fn all(&self) -> Option<&super::Pipelines> {
        match self {
            Pipelines::All(pipelines) => Some(pipelines),
            Pipelines::Interface(_) | Pipelines::None => None,
        }
    }
}

// Borrow the fields we need from the renderer so that the GpuProfiler can be
// disjointly borrowed mutably
struct RendererBorrow<'frame> {
    shadow: Option<&'frame super::Shadow>,
    pipelines: Pipelines<'frame>,
    locals: &'frame super::locals::Locals,
    views: &'frame super::Views,
    pipeline_modes: &'frame super::super::PipelineModes,
    quad_index_buffer_u16: &'frame Buffer<u16>,
    quad_index_buffer_u32: &'frame Buffer<u32>,
}

pub struct Drawer<'frame> {
    encoder:  &'frame mut wgpu::CommandEncoder,
    borrow: RendererBorrow<'frame>,
    swap_tex: &'frame wgpu::TextureView,
    globals: &'frame GlobalsBindGroup,
}

impl<'frame> Drawer<'frame> {
    pub fn new(
        encoder: &'frame mut wgpu::CommandEncoder,
        renderer: &'frame mut Renderer,
        swap_tex: &'frame wgpu::TextureView,
        globals: &'frame GlobalsBindGroup,
    ) -> Self {
       
        let (pipelines, shadow) = match &renderer.state {
            super::State::Interface { interface_pipelines, .. } => (Pipelines::Interface(interface_pipelines), None),
            super::State::Complete {
                pipelines, shadow, ..
            } => (Pipelines::All(pipelines), Some(shadow)),
            super::State::Nothing => (Pipelines::None, None),
        };

        let borrow = RendererBorrow {
            shadow,
            pipelines,
            locals: &renderer.locals,
            views: &renderer.views,
            pipeline_modes: &renderer.pipeline_modes,
            quad_index_buffer_u16: &renderer.quad_index_buffer_u16,
            quad_index_buffer_u32: &renderer.quad_index_buffer_u32,
        };


        Self {
            encoder,
            borrow,
            swap_tex,
            globals,
        }
    }

    /// Get the pipeline modes.
    pub fn pipeline_modes(&self) -> &super::super::PipelineModes { self.borrow.pipeline_modes }

    /// Returns None if the shadow renderer is not enabled at some level or the
    /// pipelines are not available yet
    pub fn shadow_pass(&mut self) -> Option<ShadowPassDrawer> {
        if !self.borrow.pipeline_modes.shadow.is_map() {
            return None;
        }

        if let ShadowMap::Enabled(ref shadow_renderer) = self.borrow.shadow?.map {

            let mut render_pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &shadow_renderer.directed_depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            render_pass.set_bind_group(0, &self.globals.bind_group, &[]);

            Some(ShadowPassDrawer {
                render_pass: Box::new(render_pass),
                borrow: &self.borrow,
                shadow_renderer,
            })
        } else {
            None
        }
    }

    /// Returns None if all the pipelines are not available
    pub fn first_pass(&mut self) -> Option<FirstPassDrawer> {
        let pipelines = self.borrow.pipelines.all()?;
        // Note: this becomes Some once pipeline creation is complete even if shadows
        // are not enabled
        let shadow = self.borrow.shadow.unwrap();

        let mut render_pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("first pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &self.borrow.views.tgt_color,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: true,
                    },
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.borrow.views.tgt_depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

        render_pass.set_bind_group(0, &self.globals.bind_group, &[]);
        render_pass.set_bind_group(1, &shadow.bind.bind_group, &[]);

        Some(FirstPassDrawer {
            render_pass: Box::new(render_pass),
            borrow: &self.borrow,
            pipelines,
            globals: self.globals,
            shadows: &shadow.bind,
            col_lights: None,
        })
    }

    /// Returns None if the clouds pipeline is not available
    pub fn second_pass(&mut self) -> Option<SecondPassDrawer> {
        let pipeline = &self.borrow.pipelines.all()?.clouds;

        let mut render_pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("second pass (clouds)"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &self.borrow.views.tgt_color_pp,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

        render_pass.set_bind_group(0, &self.globals.bind_group, &[]);

        Some(SecondPassDrawer {
            render_pass: Box::new(render_pass),
            borrow: &self.borrow,
            pipeline,
        })
    }

    /// To be ran between the second pass and the third pass
    /// does nothing if the ingame pipelines are not yet ready
    /// does nothing if bloom is disabled
    pub fn run_bloom_passes(&mut self) {
        let locals = &self.borrow.locals;
        let views = &self.borrow.views;

        let bloom_pipelines = match self.borrow.pipelines.all() {
            Some(super::Pipelines { bloom: Some(p), .. }) => p,
            _ => return,
        };

        // TODO: consider consolidating optional bloom bind groups and optional pipeline
        // into a single structure?
        let (bloom_tgts, bloom_binds) =
            match views.bloom_tgts.as_ref().zip(locals.bloom_binds.as_ref()) {
                Some((t, b)) => (t, b),
                None => return,
            };

        let mut run_bloom_pass = |bind, view, label: String, pipeline, load| {
            let pass_label = format!("bloom {} pass", label);
            let mut render_pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&pass_label),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    resolve_target: None,
                    view,
                    ops: wgpu::Operations { store: true, load },
                }],
                depth_stencil_attachment: None,
            });

            render_pass.set_bind_group(0, bind, &[]);
            render_pass.set_pipeline(pipeline);
            render_pass.draw(0..3, 0..1);
        };

        // Downsample filter passes
        (0..bloom::NUM_SIZES - 1).for_each(|index| {
            let bind = &bloom_binds[index].bind_group;
            let view = &bloom_tgts[index + 1];
            // Do filtering during the first downsample
            // NOTE: We currently blur all things without filtering by brightness.
            // This is left in for those that might want to experminent with filtering by
            // brightness, and it is used to filter out NaNs/Infs that would infect all the
            // pixels they are blurred with.
            let (label, pipeline) = if index == 0 {
                (
                    format!("downsample filtered {}", index + 1),
                    &bloom_pipelines.downsample_filtered,
                )
            } else {
                (
                    format!("downsample {}", index + 1),
                    &bloom_pipelines.downsample,
                )
            };
            run_bloom_pass(
                bind,
                view,
                label,
                pipeline,
                wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            );
        });

        // Upsample filter passes
        (0..bloom::NUM_SIZES - 1).for_each(|index| {
            let bind = &bloom_binds[bloom::NUM_SIZES - 1 - index].bind_group;
            let view = &bloom_tgts[bloom::NUM_SIZES - 2 - index];
            let label = format!("upsample {}", index + 1);
            run_bloom_pass(
                bind,
                view,
                label,
                &bloom_pipelines.upsample,
                if index + 2 == bloom::NUM_SIZES {
                    // Clear for the final image since that is just stuff from the pervious frame.
                    wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                } else {
                    // Add to less blurred images to get gradient of blur instead of a smudge>
                    // https://catlikecoding.com/unity/tutorials/advanced-rendering/bloom/
                    wgpu::LoadOp::Load
                },
            );
        });
    }

    pub fn third_pass(&mut self) -> ThirdPassDrawer {
        
        let mut render_pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("third pass (postprocess + ui)"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                // If a screenshot was requested render to that as an intermediate texture
                // instead
                view: self.swap_tex,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });

        render_pass.set_bind_group(0, &self.globals.bind_group, &[]);

        ThirdPassDrawer {
            render_pass: Box::new(render_pass),
            borrow: &self.borrow,
        }
    }

    /// Does nothing if the shadow pipelines are not available or shadow map
    /// rendering is disabled
    pub fn draw_point_shadows<'data: 'frame>(
        &mut self,
        matrices: &[shadow::PointLightMatrix; 126],
        chunks: impl Clone
        + Iterator<Item = (&'data Model<terrain::Vertex>, &'data terrain::BoundLocals)>,
    ) {
        if !self.borrow.pipeline_modes.shadow.is_map() {
            return;
        }

        if let Some(ShadowMap::Enabled(ref shadow_renderer)) = self.borrow.shadow.map(|s| &s.map) {
            
            const STRIDE: usize = std::mem::size_of::<shadow::PointLightMatrix>();
            let data = bytemuck::cast_slice(matrices);

            for face in 0..6 {
                // TODO: view creation cost?
                let view =
                    shadow_renderer
                        .point_depth
                        .tex
                        .create_view(&wgpu::TextureViewDescriptor {
                            label: Some("Point shadow cubemap face"),
                            format: None,
                            dimension: Some(wgpu::TextureViewDimension::D2),
                            aspect: wgpu::TextureAspect::DepthOnly,
                            base_mip_level: 0,
                            mip_level_count: None,
                            base_array_layer: face,
                            array_layer_count: NonZeroU32::new(1),
                        });

                let label = format!("point shadow face-{} pass", face);
                let mut render_pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some(&label),
                        color_attachments: &[],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: true,
                            }),
                            stencil_ops: None,
                        }),
                    });

                render_pass.set_pipeline(&shadow_renderer.point_pipeline.pipeline);
                set_quad_index_buffer::<terrain::Vertex>(&mut render_pass, &self.borrow);
                render_pass.set_bind_group(0, &self.globals.bind_group, &[]);

                (0../*20*/1).for_each(|point_light| {
                    render_pass.set_push_constants(
                        wgpu::ShaderStages::all(),
                        0,
                        &data[(6 * (point_light + 1) * STRIDE + face as usize * STRIDE)
                            ..(6 * (point_light + 1) * STRIDE + (face + 1) as usize * STRIDE)],
                    );
                    chunks.clone().for_each(|(model, locals)| {
                        render_pass.set_bind_group(1, &locals.bind_group, &[]);
                        render_pass.set_vertex_buffer(0, model.buf().slice(..));
                        render_pass.draw_indexed(0..model.len() as u32 / 4 * 6, 0, 0..1);
                    });
                });
            }
        }
    }

    /// Clear all the shadow textures, useful if directed shadows (shadow_pass)
    /// and point light shadows (draw_point_shadows) are unused and thus the
    /// textures will otherwise not be cleared after either their
    /// initialization or their last use
    /// NOTE: could simply use the above passes except `draw_point_shadows`
    /// requires an array of matrices that could be a pain to construct
    /// simply for clearing
    ///
    /// Does nothing if the shadow pipelines are not available (although they
    /// aren't used here they are needed for the ShadowMap to exist)
    pub fn clear_shadows(&mut self) {
        if let Some(ShadowMap::Enabled(ref shadow_renderer)) = self.borrow.shadow.map(|s| &s.map) {
            
            let _ = self.encoder.begin_render_pass(
                &wgpu::RenderPassDescriptor {
                    label: Some("clear directed shadow pass"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &shadow_renderer.directed_depth.view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: true,
                        }),
                        stencil_ops: None,
                    }),
                },
            );

            for face in 0..6 {
                // TODO: view creation cost?
                let view =
                    shadow_renderer
                        .point_depth
                        .tex
                        .create_view(&wgpu::TextureViewDescriptor {
                            label: Some("Point shadow cubemap face"),
                            format: None,
                            dimension: Some(wgpu::TextureViewDimension::D2),
                            aspect: wgpu::TextureAspect::DepthOnly,
                            base_mip_level: 0,
                            mip_level_count: None,
                            base_array_layer: face,
                            array_layer_count: NonZeroU32::new(1),
                        });

                let label = format!("clear point shadow face-{} pass", face);
                let _ = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some(&label),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: true,
                        }),
                        stencil_ops: None,
                    }),
                });
            }
        }
    }
}

// Shadow pass
pub struct ShadowPassDrawer<'pass> {
    render_pass: Box<wgpu::RenderPass<'pass>>,
    borrow: &'pass RendererBorrow<'pass>,
    shadow_renderer: &'pass ShadowMapRenderer,
}

impl<'pass> ShadowPassDrawer<'pass> {
    pub fn init_figure_shadows(&mut self) {

        let render_pass = self.render_pass.as_mut();
        render_pass.set_pipeline(&self.shadow_renderer.figure_directed_pipeline.pipeline);
        set_quad_index_buffer::<terrain::Vertex>(render_pass, self.borrow);
    }

    pub fn draw_figure_shadows<'data: 'pass>(
        &mut self,
        model: SubModel<'data, terrain::Vertex>,
        locals: &'data figure::BoundLocals,
    ) {

        let render_pass = self.render_pass.as_mut();
        render_pass.set_bind_group(1, &locals.bind_group, &[]);
        render_pass.set_vertex_buffer(0, model.buf());
        render_pass.draw_indexed(0..model.len() as u32 / 4 * 6, 0, 0..1);
    }


    pub fn init_terrain_shadows(&mut self) {

        let render_pass = self.render_pass.as_mut();
        render_pass.set_pipeline(&self.shadow_renderer.terrain_directed_pipeline.pipeline);
        set_quad_index_buffer::<terrain::Vertex>(render_pass, self.borrow);
    }

    pub fn draw_terrain_shadows<'data: 'pass>(
        &mut self,
        model: &'data Model<terrain::Vertex>,
        locals: &'data terrain::BoundLocals,
    ) {
        let render_pass = self.render_pass.as_mut();
        render_pass.set_bind_group(1, &locals.bind_group, &[]);
        render_pass.set_vertex_buffer(0, model.buf().slice(..));
        render_pass.draw_indexed(0..model.len() as u32 / 4 * 6, 0, 0..1);
    }
}

// First pass
pub struct FirstPassDrawer<'pass> {
    pub(super) render_pass: Box<wgpu::RenderPass<'pass>>,
    borrow: &'pass RendererBorrow<'pass>,
    pipelines: &'pass super::Pipelines,
    globals: &'pass GlobalsBindGroup,
    shadows: &'pass ShadowTexturesBindGroup,
    col_lights: Option<&'pass Arc<ColLights<terrain::Locals>>>,
}

impl<'pass> FirstPassDrawer<'pass> {
    pub fn draw_skybox<'data: 'pass>(&mut self, model: &'data Model<skybox::Vertex>) {

        let render_pass = self.render_pass.as_mut();

        render_pass.set_pipeline(&self.pipelines.skybox.pipeline);
        set_quad_index_buffer::<skybox::Vertex>(render_pass, self.borrow);

        render_pass.set_vertex_buffer(0, model.buf().slice(..));
        render_pass.draw(0..model.len() as u32, 0..1);
    }

    pub fn init_debug(&mut self) {

        let render_pass = self.render_pass.as_mut();
        render_pass.set_pipeline(&self.pipelines.debug.pipeline);
        set_quad_index_buffer::<debug::Vertex>(render_pass, self.borrow);
    }

    pub fn draw_debug<'data: 'pass>(
        &mut self,
        model: &'data Model<debug::Vertex>,
        locals: &'data debug::BoundLocals,
    ) {
        let render_pass = self.render_pass.as_mut();
        render_pass.set_bind_group(1, &locals.bind_group, &[]);
        render_pass.set_vertex_buffer(0, model.buf().slice(..));
        render_pass.draw(0..model.len() as u32, 0..1);
    }

    pub fn drop_debug<'data: 'pass>(&mut self){
        // Maintain that the shadow bind group is set in
        // slot 1 by default during the main pass
        let render_pass = self.render_pass.as_mut();
        render_pass.set_bind_group(1, &self.shadows.bind_group, &[]);
    }

    pub fn draw_lod_terrain<'data: 'pass>(&mut self, model: &'data Model<lod_terrain::Vertex>) {

        let render_pass = self.render_pass.as_mut();

        render_pass.set_pipeline(&self.pipelines.lod_terrain.pipeline);
        set_quad_index_buffer::<lod_terrain::Vertex>(render_pass, self.borrow);
        render_pass.set_vertex_buffer(0, model.buf().slice(..));
        render_pass.draw_indexed(0..model.len() as u32 / 4 * 6, 0, 0..1);
    }

    pub fn init_figures(&mut self) {
        
        let render_pass = self.render_pass.as_mut();
        render_pass.set_pipeline(&self.pipelines.figure.pipeline);
        // Note: figures use the same vertex type as the terrain
        set_quad_index_buffer::<terrain::Vertex>(render_pass, self.borrow);
    }
    
    pub fn draw_figures<'data: 'pass>(
        &mut self,
        model: SubModel<'data, terrain::Vertex>,
        locals: &'data figure::BoundLocals,
        // TODO: don't rebind this every time once they are shared between figures
        col_lights: &'data ColLights<figure::Locals>,
    ) {

        let render_pass = self.render_pass.as_mut();
        render_pass.set_bind_group(2, &col_lights.bind_group, &[]);
        render_pass.set_bind_group(3, &locals.bind_group, &[]);
        render_pass.set_vertex_buffer(0, model.buf());
        render_pass.draw_indexed(0..model.len() as u32 / 4 * 6, 0, 0..1);
    }

    pub fn drop_figures<'data: 'pass>(&mut self){

    }


    pub fn init_terrain(&mut self) {

        let render_pass = self.render_pass.as_mut();
        render_pass.set_pipeline(&self.pipelines.terrain.pipeline);
        set_quad_index_buffer::<terrain::Vertex>(render_pass, self.borrow);
    }

    pub fn draw_terrain<'data: 'pass>(
        &mut self,
        model: &'data Model<terrain::Vertex>,
        col_lights: &'data Arc<ColLights<terrain::Locals>>,
        locals: &'data terrain::BoundLocals,
    ) {
        let render_pass = self.render_pass.as_mut();

        if self.col_lights
            // Check if we are still using the same atlas texture as the previous drawn
            // chunk
            .filter(|current_col_lights| Arc::ptr_eq(current_col_lights, col_lights))
            .is_none()
        {
            render_pass.set_bind_group(2, &col_lights.bind_group, &[]);
            self.col_lights = Some(col_lights);
        };

        render_pass.set_bind_group(3, &locals.bind_group, &[]);
        render_pass.set_vertex_buffer(0, model.buf().slice(..));
        render_pass.draw_indexed(0..model.len() as u32 / 4 * 6, 0, 0..1);
    }


    pub fn init_particles(&mut self)  {

        let render_pass = self.render_pass.as_mut();
        render_pass.set_pipeline(&self.pipelines.particle.pipeline);
        set_quad_index_buffer::<particle::Vertex>(render_pass, self.borrow);
    }

    pub fn draw_particles<'data: 'pass>(
        &mut self,
        model: &'data Model<particle::Vertex>,
        instances: &'data Instances<particle::Instance>,
    ) {
        let render_pass = self.render_pass.as_mut();
        render_pass.set_vertex_buffer(0, model.buf().slice(..));
        render_pass.set_vertex_buffer(1, instances.buf().slice(..));
        render_pass.draw_indexed(0..model.len() as u32 / 4 * 6, 0, 0..instances.count() as u32);
    }

   
    pub fn init_sprites<'data: 'pass>(
        &mut self,
        globals: &'data sprite::SpriteGlobalsBindGroup,
        col_lights: &'data ColLights<sprite::Locals>,
    ) {

        let render_pass = self.render_pass.as_mut();
        render_pass.set_pipeline(&self.pipelines.sprite.pipeline);
        set_quad_index_buffer::<sprite::Vertex>(render_pass, self.borrow);
        render_pass.set_bind_group(0, &globals.bind_group, &[]);
        render_pass.set_bind_group(2, &col_lights.bind_group, &[]);
    }

    pub fn draw_sprites<'data: 'pass>(
        &mut self,
        terrain_locals: &'data terrain::BoundLocals,
        instances: &'data Instances<sprite::Instance>,
    ) {

        let render_pass = self.render_pass.as_mut();
        render_pass.set_bind_group(3, &terrain_locals.bind_group, &[]);

        render_pass.set_vertex_buffer(0, instances.buf().slice(..));
        render_pass.draw_indexed(
            0..sprite::VERT_PAGE_SIZE / 4 * 6,
            0,
            0..instances.count() as u32,
        );
    }

    pub fn drop_sprites<'data: 'pass>(&mut self) {
        // Reset to regular globals
        let render_pass = self.render_pass.as_mut();
        render_pass.set_bind_group(0, &self.globals.bind_group, &[]);
    }

    pub fn init_fluid(&mut self) {
        let render_pass = self.render_pass.as_mut();
        render_pass.set_pipeline(&self.pipelines.fluid.pipeline);
        set_quad_index_buffer::<fluid::Vertex>(render_pass, self.borrow);
    }

    pub fn draw_fluid<'data: 'pass>(
        &mut self,
        model: &'data Model<fluid::Vertex>,
        locals: &'data terrain::BoundLocals,
    ) {
        let render_pass = self.render_pass.as_mut();
        render_pass.set_vertex_buffer(0, model.buf().slice(..));
        render_pass.set_bind_group(2, &locals.bind_group, &[]);
        render_pass.draw_indexed(0..model.len() as u32 / 4 * 6, 0, 0..1);
    }

    
    pub fn drop_fluid<'data: 'pass>(&mut self) {
       
    }
}


// Second pass: clouds
pub struct SecondPassDrawer<'pass> {
    render_pass: Box<wgpu::RenderPass<'pass>>,
    borrow: &'pass RendererBorrow<'pass>,
    pipeline: &'pass clouds::CloudsPipeline,
}

impl<'pass> SecondPassDrawer<'pass> {
    pub fn draw_clouds(&mut self) {

        let render_pass = self.render_pass.as_mut();
        render_pass.set_pipeline(&self.pipeline.pipeline);
        render_pass.set_bind_group(1, &self.borrow.locals.clouds_bind.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

/// Third pass: postprocess + ui
pub struct ThirdPassDrawer<'pass> {
    render_pass: Box<wgpu::RenderPass<'pass>>,
    borrow: &'pass RendererBorrow<'pass>,
}

impl<'pass> ThirdPassDrawer<'pass> {
    /// Does nothing if the postprocess pipeline is not available
    pub fn draw_postprocess(&mut self) {
        let postprocess = match self.borrow.pipelines.all() {
            Some(p) => &p.postprocess,
            None => return,
        };

        let render_pass = self.render_pass.as_mut();
        render_pass.set_pipeline(&postprocess.pipeline);
        render_pass.set_bind_group(1, &self.borrow.locals.postprocess_bind.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    /// Returns None if the UI pipeline is not available (note: this should
    /// never be the case for now)
    pub fn init_ui(&mut self)  {

        let ui = self.borrow.pipelines.ui().unwrap();

        let render_pass = self.render_pass.as_mut();
        render_pass.set_pipeline(&ui.pipeline);
        set_quad_index_buffer::<ui::Vertex>(render_pass, self.borrow);
    }

    /// Set vertex buffer, initial scissor, and locals
    /// These can be changed later but this ensures that they don't have to be
    /// set with every draw call
    pub fn ui_prepare<'data: 'pass>(
        &mut self,
        locals: &'data ui::BoundLocals,
        buf: &'data DynamicModel<ui::Vertex>,
        scissor: Aabr<u16>,
    )  {
        // Prepare
        self.ui_set_locals(locals);
        self.ui_set_model(buf);
        self.ui_set_scissor(scissor);
    }

    pub fn ui_set_locals<'data: 'pass>(&mut self, locals: &'data ui::BoundLocals) {

        let render_pass = self.render_pass.as_mut();
        render_pass.set_bind_group(1, &locals.bind_group, &[]);

    }

    pub fn ui_set_model<'data: 'pass>(&mut self, model: &'data DynamicModel<ui::Vertex>) {

        let render_pass = self.render_pass.as_mut();
        render_pass.set_vertex_buffer(0, model.buf().slice(..))
    }

    pub fn ui_set_scissor(&mut self, scissor: Aabr<u16>) {
        let Aabr { min, max } = scissor;
        let render_pass = self.render_pass.as_mut();
        render_pass.set_scissor_rect(
            min.x as u32,
            min.y as u32,
            (max.x - min.x) as u32,
            (max.y - min.y) as u32,
        );
    }

    pub fn ui_draw<'data: 'pass>(&mut self, texture: &'data ui::TextureBindGroup, verts: Range<u32>) {
        let render_pass = self.render_pass.as_mut();
        render_pass.set_bind_group(2, &texture.bind_group, &[]);
        render_pass.draw(verts, 0..1);
    }
}


fn set_quad_index_buffer<'a, V: super::super::Vertex>(
    pass: &mut wgpu::RenderPass<'a>,
    borrow: &RendererBorrow<'a>,
) {
    if let Some(format) = V::QUADS_INDEX {
        let slice = match format {
            wgpu::IndexFormat::Uint16 => borrow.quad_index_buffer_u16.buf.slice(..),
            wgpu::IndexFormat::Uint32 => borrow.quad_index_buffer_u32.buf.slice(..),
        };

        pass.set_index_buffer(slice, format);
    }
}
