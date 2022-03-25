mod binding;
pub(super) mod drawer;
// Consts and bind groups for post-process and clouds
mod locals;
mod pipeline_creation;
mod shadow_map;

use locals::Locals;
use pipeline_creation::{
    Pipelines, ShadowPipelines, InterfacePipelines, IngamePipelines,
};
use shadow_map::{ShadowMap, ShadowMapRenderer};

use super::{
    buffer::Buffer,
    consts::Consts,
    instances::Instances,
    mesh::Mesh,
    model::{DynamicModel, Model},
    pipelines::{
        blit, bloom, clouds, debug, figure, postprocess, shadow, sprite, terrain, ui,
        GlobalsBindGroup, GlobalsLayouts, ShadowTexturesBindGroup,
    },
    texture::Texture,
    AaMode, AddressMode, FilterMode, OtherModes, PipelineModes, RenderError, RenderMode,
    ShadowMapMode, ShadowMode, Vertex,
};
use common::assets::{self, AssetExt};
use core::convert::TryFrom;
use std::sync::Arc;
use vek::*;

// TODO: yeet this somewhere else
/// A type representing data that can be converted to an immutable texture map
/// of ColLight data (used for texture atlases created during greedy meshing).
// TODO: revert to u16
pub type ColLightInfo = (Vec<[u8; 4]>, Vec2<u16>);

const QUAD_INDEX_BUFFER_U16_START_VERT_LEN: u16 = 3000;
const QUAD_INDEX_BUFFER_U32_START_VERT_LEN: u32 = 3000;

/// A type that stores all the layouts associated with this renderer that never
/// change when the RenderMode is modified.
struct ImmutableLayouts {
    global: GlobalsLayouts,

    debug: debug::DebugLayout,
    figure: figure::FigureLayout,
    shadow: shadow::ShadowLayout,
    sprite: sprite::SpriteLayout,
    terrain: terrain::TerrainLayout,
    clouds: clouds::CloudsLayout,
    bloom: bloom::BloomLayout,
    ui: ui::UiLayout,
    blit: blit::BlitLayout,
}

/// A type that stores all the layouts associated with this renderer.
struct Layouts {
    immutable: Arc<ImmutableLayouts>,

    postprocess: Arc<postprocess::PostProcessLayout>,
}

impl core::ops::Deref for Layouts {
    type Target = ImmutableLayouts;

    fn deref(&self) -> &Self::Target { &self.immutable }
}

/// Render target views
struct Views {
    // NOTE: unused for now, maybe... we will want it for something
    _win_depth: wgpu::TextureView,

    tgt_color: wgpu::TextureView,
    tgt_depth: wgpu::TextureView,

    bloom_tgts: Option<[wgpu::TextureView; bloom::NUM_SIZES]>,
    // TODO: rename
    tgt_color_pp: wgpu::TextureView,
}

/// Shadow rendering textures, layouts, pipelines, and bind groups
struct Shadow {
    map: ShadowMap,
    bind: ShadowTexturesBindGroup,
}

/// Represent two states of the renderer:
/// 1. Only interface pipelines created
/// 2. All of the pipelines have been created
#[allow(clippy::large_enum_variant)] // They are both pretty large
enum State {
    // NOTE: this is used as a transient placeholder for moving things out of State temporarily
    Nothing,
    Interface {
        interface_pipelines: InterfacePipelines,
        ingame_pipelines: IngamePipelines,
	    shadow_views: Option<(Texture, Texture)>,
        shadow_pipelines: ShadowPipelines,
    },
    Complete {
        pipelines: Pipelines,
        shadow: Shadow,
    },
}

/// A type that encapsulates rendering state. `Renderer` is central to Voxygen's
/// rendering subsystem and contains any state necessary to interact with the
/// GPU, along with pipeline state objects (PSOs) needed to renderer different
/// kinds of models to the screen.
pub struct Renderer {
    pub device: Arc<wgpu::Device>,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface,
    pub sc_desc: wgpu::SurfaceConfiguration,

    sampler: wgpu::Sampler,
    depth_sampler: wgpu::Sampler,

    state: State,
    // Some if there is a pending need to recreate the pipelines (e.g. RenderMode change or shader
    // hotloading)
    recreation_pending: Option<PipelineModes>,

    layouts: Layouts,
    // Note: we keep these here since their bind groups need to be updated if we resize the
    // color/depth textures
    locals: Locals,
    views: Views,
    noise_tex: Texture,

    quad_index_buffer_u16: Buffer<u16>,
    quad_index_buffer_u32: Buffer<u32>,

    pipeline_modes: PipelineModes,
    other_modes: OtherModes,
    pub resolution: Vec2<u32>,

    // This checks is added because windows resizes the window to 0,0 when
    // minimizing and this causes a bunch of validation errors
    is_minimized: bool,
}

impl Renderer {
    /// Create a new `Renderer` from a variety of backend-specific components
    /// and the window targets.
    pub fn new(
        window: &winit::window::Window,
        mode: RenderMode,
        runtime: &tokio::runtime::Runtime,
    ) -> Result<Self, RenderError> {
        let (pipeline_modes, other_modes) = mode.split();
       

        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let dims = window.inner_size();

        // This is unsafe because the window handle must be valid, if you find a way to
        // have an invalid winit::Window then you have bigger issues
        #[allow(unsafe_code)]
        let surface = unsafe { instance.create_surface(window) };

        let adapter = runtime.block_on(instance
            .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            // Request an adapter which can render to our surface
            compatible_surface: Some(&surface),
        }))
        .expect("Failed to find an appropriate adapter");


        let (device, queue) = runtime.block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: /*  wasm 不支持
                      wgpu::Features::DEPTH_CLIP_CONTROL
                    | wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER
                    | wgpu::Features::PUSH_CONSTANTS
                    | */adapter.features(),
                limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            },
            None,
        ))?;

        let format = surface.get_preferred_format(&adapter)
                .expect("No supported swap chain format found");
        log::info!("Using {:?} as the swapchain format", format);

        let sc_desc = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: dims.width,
            height: dims.height,
            present_mode: other_modes.present_mode.into(),
        };

        surface.configure(&device, &sc_desc);

        log::info!("downlevel_properties:{:?}", &adapter.get_downlevel_properties());


        let shadow_views = ShadowMap::create_shadow_views(
            &device,
            (dims.width, dims.height),
            &ShadowMapMode::try_from(pipeline_modes.shadow).unwrap_or_default(),
        )
        .map_err(|err| {
            log::warn!("Could not create shadow map views: {:?}", err);
        })
        .ok();

        let layouts = {

            log::info!("init global layout");
            let global = GlobalsLayouts::new(&device);

            log::info!("init debug layout");
            let debug = debug::DebugLayout::new(&device);

            log::info!("init figure layout");
            let figure = figure::FigureLayout::new(&device);

            log::info!("init shadow layout");
            let shadow = shadow::ShadowLayout::new(&device);

            log::info!("init sprite layout");
            let sprite = sprite::SpriteLayout::new(&device);

            log::info!("init terrain layout");
            let terrain = terrain::TerrainLayout::new(&device);

            log::info!("init clouds layout");
            let clouds = clouds::CloudsLayout::new(&device);

            log::info!("init bloom layout");
            let bloom = bloom::BloomLayout::new(&device);

            log::info!("init postprocess layout");
            let postprocess = Arc::new(postprocess::PostProcessLayout::new(&device, &pipeline_modes));

            log::info!("init ui layout");
            let ui = ui::UiLayout::new(&device);

            log::info!("init blit layout");
            let blit = blit::BlitLayout::new(&device);

            let immutable = Arc::new(ImmutableLayouts {
                global,
                debug,
                figure,
                shadow,
                sprite,
                terrain,
                clouds,
                bloom,
                ui,
                blit,
            });

            Layouts {
                immutable,
                postprocess,
            }
        };

        // Arcify the device
        let device = Arc::new(device);

        let (interface_pipelines, ingame_pipelines, shadow_pipelines) = pipeline_creation::initial_create_pipelines(
            Arc::clone(&device),
            Layouts {
                immutable: Arc::clone(&layouts.immutable),
                postprocess: Arc::clone(&layouts.postprocess),
            },
            pipeline_modes.clone(),
            sc_desc.clone(), // Note: cheap clone
        )?;

        let state = State::Interface {
            interface_pipelines,
            ingame_pipelines,
            shadow_views,
            shadow_pipelines,
        };

        let (views, bloom_sizes) = Self::create_rt_views(
            &device,
            (dims.width, dims.height),
            &pipeline_modes,
            &other_modes,
        );

        let create_sampler = |filter| {

            let sampler_info = &wgpu::SamplerDescriptor {
                label: None,
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: filter,
                min_filter: filter,
                mipmap_filter: wgpu::FilterMode::Nearest,
                compare: None,
                ..Default::default()
            };
            log::warn!("create_sampler: {:?}",&sampler_info);
            device.create_sampler(sampler_info)
        };

        let sampler = create_sampler(wgpu::FilterMode::Linear);
        let depth_sampler = create_sampler(wgpu::FilterMode::Nearest);

        let noise_tex = Texture::new(
            &device,
            &queue,
            &assets::Image::load_expect("voxygen.texture.noise").read().0,
            Some(wgpu::FilterMode::Linear),
            Some(wgpu::AddressMode::Repeat),
        )?;

        let clouds_locals =
            Self::create_consts_inner(&device, &queue, &[clouds::Locals::default()]);
        let postprocess_locals =
            Self::create_consts_inner(&device, &queue, &[postprocess::Locals::default()]);

        let locals = Locals::new(
            &device,
            &layouts,
            clouds_locals,
            postprocess_locals,
            &views.tgt_color,
            &views.tgt_depth,
            views.bloom_tgts.as_ref().map(|tgts| locals::BloomParams {
                locals: bloom_sizes.map(|size| {
                    Self::create_consts_inner(&device, &queue, &[bloom::Locals::new(size)])
                }),
                src_views: [&views.tgt_color_pp, &tgts[1], &tgts[2], &tgts[3], &tgts[4]],
                final_tgt_view: &tgts[0],
            }),
            &views.tgt_color_pp,
            &sampler,
            &depth_sampler,
        );

        let quad_index_buffer_u16 =
            create_quad_index_buffer_u16(&device, QUAD_INDEX_BUFFER_U16_START_VERT_LEN as usize);
        let quad_index_buffer_u32 =
            create_quad_index_buffer_u32(&device, QUAD_INDEX_BUFFER_U32_START_VERT_LEN as usize);

        Ok(Self {
            device,
            queue,
            surface,
            sc_desc,

            state,
            recreation_pending: None,

            layouts,
            locals,
            views,

            sampler,
            depth_sampler,
            noise_tex,

            quad_index_buffer_u16,
            quad_index_buffer_u32,

            pipeline_modes,
            other_modes,
            resolution: Vec2::new(dims.width, dims.height),

            is_minimized: false,
        })
    }

    /// Change the render mode.
    pub fn set_render_mode(&mut self, mode: RenderMode) -> Result<(), RenderError> {
        let (pipeline_modes, other_modes) = mode.split();

        if self.other_modes != other_modes {
            self.other_modes = other_modes;

            // Update present mode in swap chain descriptor
            self.sc_desc.present_mode = self.other_modes.present_mode.into();

            // Recreate render target
            self.on_resize(self.resolution);
        }

        Ok(())
    }

    /// Get the pipelines mode.
    pub fn pipeline_modes(&self) -> &PipelineModes { &self.pipeline_modes }

    /// Resize internal render targets to match window render target dimensions.
    pub fn on_resize(&mut self, dims: Vec2<u32>) {
        // Avoid panics when creating texture with w,h of 0,0.
        if dims.x != 0 && dims.y != 0 {
            self.is_minimized = false;
            // Resize swap chain
            self.resolution = dims;
            self.sc_desc.width = dims.x;
            self.sc_desc.height = dims.y;

            self.surface.configure(&self.device, &self.sc_desc);
            //self.swap_chain = self.device.create_swap_chain(&self.surface, &self.sc_desc);

            // Resize other render targets
            let (views, bloom_sizes) = Self::create_rt_views(
                &self.device,
                (dims.x, dims.y),
                &self.pipeline_modes,
                &self.other_modes,
            );
            self.views = views;

            // appease borrow check (TODO: remove after Rust 2021)
            let device = &self.device;
            let queue = &self.queue;
            let views = &self.views;
            let bloom_params = self
                .views
                .bloom_tgts
                .as_ref()
                .map(|tgts| locals::BloomParams {
                    locals: bloom_sizes.map(|size| {
                        Self::create_consts_inner(device, queue, &[bloom::Locals::new(size)])
                    }),
                    src_views: [&views.tgt_color_pp, &tgts[1], &tgts[2], &tgts[3], &tgts[4]],
                    final_tgt_view: &tgts[0],
                });

            self.locals.rebind(
                &self.device,
                &self.layouts,
                &self.views.tgt_color,
                &self.views.tgt_depth,
                bloom_params,
                &self.views.tgt_color_pp,
                &self.sampler,
                &self.depth_sampler,
            );

            // Get mutable reference to shadow views out of the current state
            let shadow_views = match &mut self.state {
                State::Interface { shadow_views, .. } => {
                    shadow_views.as_mut().map(|s| (&mut s.0, &mut s.1))
                },
                State::Complete {
                    shadow:
                        Shadow {
                            map: ShadowMap::Enabled(shadow_map),
                            ..
                        },
                    ..
                } => Some((&mut shadow_map.point_depth, &mut shadow_map.directed_depth)),
                State::Complete { .. } => None,
                State::Nothing => None, // Should never hit this
            };

            if let (Some((point_depth, directed_depth)), ShadowMode::Map(mode)) =
                (shadow_views, self.pipeline_modes.shadow)
            {
                match ShadowMap::create_shadow_views(&self.device, (dims.x, dims.y), &mode) {
                    Ok((new_point_depth, new_directed_depth)) => {
                        *point_depth = new_point_depth;
                        *directed_depth = new_directed_depth;
                        // Recreate the shadow bind group if needed
                        if let State::Complete {
                            shadow:
                                Shadow {
                                    bind,
                                    map: ShadowMap::Enabled(shadow_map),
                                    ..
                                },
                            ..
                        } = &mut self.state
                        {
                            *bind = self.layouts.global.bind_shadow_textures(
                                &self.device,
                                &shadow_map.point_depth,
                                &shadow_map.directed_depth,
                            );
                        }
                    },
                    Err(err) => {
                        log::warn!("Could not create shadow map views: {:?}", err);
                    },
                }
            }
        } else {
            self.is_minimized = true;
        }
    }

    pub fn maintain(&self) {
        if self.is_minimized {
            self.queue.submit(std::iter::empty());
        }

        self.device.poll(wgpu::Maintain::Poll)
    }

    /// Create render target views
    fn create_rt_views(
        device: &wgpu::Device,
        size: (u32, u32),
        pipeline_modes: &PipelineModes,
        other_modes: &OtherModes,
    ) -> (Views, [Vec2<f32>; bloom::NUM_SIZES]) {
        let upscaled = Vec2::<u32>::from(size)
            .map(|e| (e as f32 * other_modes.upscale_mode.factor) as u32)
            .into_tuple();
        let (width, height, sample_count) = match pipeline_modes.aa {
            AaMode::None | AaMode::Fxaa => (upscaled.0, upscaled.1, 1),
            AaMode::MsaaX4 => (upscaled.0, upscaled.1, 4),
            AaMode::MsaaX8 => (upscaled.0, upscaled.1, 8),
            AaMode::MsaaX16 => (upscaled.0, upscaled.1, 16),
        };
        let levels = 1;

        let color_view = |width, height| {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: levels,
                sample_count,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            });

            tex.create_view(&wgpu::TextureViewDescriptor {
                label: None,
                format: Some(wgpu::TextureFormat::Rgba16Float),
                dimension: Some(wgpu::TextureViewDimension::D2),
                // TODO: why is this not Color?
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            })
        };

        let tgt_color_view = color_view(width, height);
        let tgt_color_pp_view = color_view(width, height);

        let mut size_shift = 0;
        // TODO: skip creating bloom stuff when it is disabled
        let bloom_sizes = [(); bloom::NUM_SIZES].map(|()| {
            // .max(1) to ensure we don't create zero sized textures
            let size = Vec2::new(width, height).map(|e| (e >> size_shift).max(1));
            size_shift += 1;
            size
        });

        let bloom_tgt_views = pipeline_modes
            .bloom
            .is_on()
            .then(|| bloom_sizes.map(|size| color_view(size.x, size.y)));

        let tgt_depth_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: levels,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        });
        let tgt_depth_view = tgt_depth_tex.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(wgpu::TextureFormat::Depth32Float),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::DepthOnly,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        let win_depth_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: levels,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        });
        // TODO: Consider no depth buffer for the final draw to the window?
        let win_depth_view = win_depth_tex.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(wgpu::TextureFormat::Depth32Float),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::DepthOnly,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        (
            Views {
                tgt_color: tgt_color_view,
                tgt_depth: tgt_depth_view,
                bloom_tgts: bloom_tgt_views,
                tgt_color_pp: tgt_color_pp_view,
                _win_depth: win_depth_view,
            },
            bloom_sizes.map(|s| s.map(|e| e as f32)),
        )
    }

    /// Get the resolution of the render target.
    pub fn resolution(&self) -> Vec2<u32> { self.resolution }

    /// Get the resolution of the shadow render target.
    pub fn get_shadow_resolution(&self) -> (Vec2<u32>, Vec2<u32>) {
        match &self.state {
            State::Interface { shadow_views, .. } => shadow_views.as_ref().map(|s| (&s.0, &s.1)),
            State::Complete {
                shadow:
                    Shadow {
                        map: ShadowMap::Enabled(shadow_map),
                        ..
                    },
                ..
            } => Some((&shadow_map.point_depth, &shadow_map.directed_depth)),
            State::Complete { .. } | State::Nothing => None,
        }
        .map(|(point, directed)| (point.get_dimensions().xy(), directed.get_dimensions().xy()))
        .unwrap_or_else(|| (Vec2::new(1, 1), Vec2::new(1, 1)))
    }

    // TODO: Seamless is potentially the default with wgpu but we need further
    // investigation into whether this is actually turned on for the OpenGL
    // backend
    //
    /// NOTE: Supported by Vulkan (by default), DirectX 10+ (it seems--it's hard
    /// to find proof of this, but Direct3D 10 apparently does it by
    /// default, and 11 definitely does, so I assume it's natively supported
    /// by DirectX itself), OpenGL 3.2+, and Metal (done by default).  While
    /// there may be some GPUs that don't quite support it correctly, the
    /// impact is relatively small, so there is no reason not to enable it where
    /// available.
    //fn enable_seamless_cube_maps() {
    //todo!()
    // unsafe {
    //     // NOTE: Currently just fail silently rather than complain if the
    // computer is on     // a version lower than 3.2, where
    // seamless cubemaps were introduced.     if !device.get_info().
    // is_version_supported(3, 2) {         return;
    //     }

    //     // NOTE: Safe because GL_TEXTURE_CUBE_MAP_SEAMLESS is supported
    // by OpenGL 3.2+     // (see https://www.khronos.org/opengl/wiki/Cubemap_Texture#Seamless_cubemap);
    //     // enabling seamless cube maps should always be safe regardless
    // of the state of     // the OpenGL context, so no further
    // checks are needed.     device.with_gl(|gl| {
    //         gl.Enable(gfx_gl::TEXTURE_CUBE_MAP_SEAMLESS);
    //     });
    // }
    //}

    /// Start recording the frame
    /// When the returned `Drawer` is dropped the recorded draw calls will be
    /// submitted to the queue
    /// If there is an intermittent issue with the swap chain then Ok(None) will
    /// be returned
    pub fn start_recording_frame<'a>(
        &'a mut self,
        globals: &'a GlobalsBindGroup,
        encoder: &'a mut wgpu::CommandEncoder,
        view: &'a wgpu::TextureView,

    ) -> Result<Option<drawer::Drawer<'a>>, RenderError> {
       
        if self.is_minimized {
            return Ok(None);
        }

        // Handle polling background pipeline creation/recreation
        // Temporarily set to nothing and then replace in the statement below
        let state = core::mem::replace(&mut self.state, State::Nothing);
        
        // If still creating initial pipelines, check if complete
        self.state = if let State::Interface {
            interface_pipelines,
            ingame_pipelines,
            shadow_views,
            shadow_pipelines,
        } = state
        {
            let pipelines = Pipelines::consolidate(interface_pipelines, ingame_pipelines);
            let shadow_map = ShadowMap::new(
                &self.device,
                &self.queue,
                shadow_pipelines.point,
                shadow_pipelines.directed,
                shadow_pipelines.figure,
                shadow_views,
            );

            let shadow_bind = {
                let (point, directed) = shadow_map.textures();
                self.layouts
                    .global
                    .bind_shadow_textures(&self.device, point, directed)
            };

            let shadow = Shadow {
                map: shadow_map,
                bind: shadow_bind,
            };
            State::Complete {
                pipelines,
                shadow,
            }
        // If recreating the pipelines, check if that is complete
        } else if let State::Complete {
            pipelines,
            mut shadow,
        } = state
        {
            // if let (
            //     Some(point_pipeline),
            //     Some(terrain_directed_pipeline),
            //     Some(figure_directed_pipeline),
            //     ShadowMap::Enabled(shadow_map),
            // ) = (
            //     shadow_pipelines.point,
            //     shadow_pipelines.directed,
            //     shadow_pipelines.figure,
            //     &mut shadow.map,
            // ) {
            //     shadow_map.point_pipeline = point_pipeline;
            //     shadow_map.terrain_directed_pipeline = terrain_directed_pipeline;
            //     shadow_map.figure_directed_pipeline = figure_directed_pipeline;
            // }

            //self.pipeline_modes = new_pipeline_modes;
            //self.layouts.postprocess = postprocess_layout;

            // trigger_on_resize = true;

            State::Complete {
                pipelines,
                shadow,
            }
        } else {
            state
        };

        // // Call on_resize to recreate render targets and their bind groups if the
        // // pipelines were recreated with a new postprocess layout and or changes in the
        // // render modes
        // if trigger_on_resize {
        //     self.on_resize(self.resolution);
        // }

        Ok(Some(drawer::Drawer::new(encoder, self, view, globals)))
    }

    /// Create a new set of constants with the provided values.
    pub fn create_consts<T: Copy + bytemuck::Pod>(&mut self, vals: &[T]) -> Consts<T> {
        Self::create_consts_inner(&self.device, &self.queue, vals)
    }

    pub fn create_consts_inner<T: Copy + bytemuck::Pod>(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vals: &[T],
    ) -> Consts<T> {
        let mut consts = Consts::new(device, vals.len());
        consts.update(queue, vals, 0);
        consts
    }

    /// Update a set of constants with the provided values.
    pub fn update_consts<T: Copy + bytemuck::Pod>(&self, consts: &mut Consts<T>, vals: &[T]) {
        consts.update(&self.queue, vals, 0)
    }

    pub fn update_clouds_locals(&mut self, new_val: clouds::Locals) {
        self.locals.clouds.update(&self.queue, &[new_val], 0)
    }

    pub fn update_postprocess_locals(&mut self, new_val: postprocess::Locals) {
        self.locals.postprocess.update(&self.queue, &[new_val], 0)
    }

    /// Create a new set of instances with the provided values.
    pub fn create_instances<T: Copy + bytemuck::Pod>(
        &mut self,
        vals: &[T],
    ) -> Result<Instances<T>, RenderError> {
        let mut instances = Instances::new(&self.device, vals.len());
        instances.update(&self.queue, vals, 0);
        Ok(instances)
    }

    /// Ensure that the quad index buffer is large enough for a quad vertex
    /// buffer with this many vertices
    pub(super) fn ensure_sufficient_index_length<V: Vertex>(
        &mut self,
        // Length of the vert buffer with 4 verts per quad
        vert_length: usize,
    ) {
        let quad_index_length = vert_length / 4 * 6;

        match V::QUADS_INDEX {
            Some(wgpu::IndexFormat::Uint16) => {
                // Make sure the global quad index buffer is large enough
                if self.quad_index_buffer_u16.len() < quad_index_length {
                    // Make sure we aren't over the max
                    if vert_length > u16::MAX as usize {
                        panic!(
                            "Vertex type: {} needs to use a larger index type, length: {}",
                            core::any::type_name::<V>(),
                            vert_length
                        );
                    }
                    self.quad_index_buffer_u16 =
                        create_quad_index_buffer_u16(&self.device, vert_length);
                }
            },
            Some(wgpu::IndexFormat::Uint32) => {
                // Make sure the global quad index buffer is large enough
                if self.quad_index_buffer_u32.len() < quad_index_length {
                    // Make sure we aren't over the max
                    if vert_length > u32::MAX as usize {
                        panic!(
                            "More than u32::MAX({}) verts({}) for type({}) using an index buffer!",
                            u32::MAX,
                            vert_length,
                            core::any::type_name::<V>()
                        );
                    }
                    self.quad_index_buffer_u32 =
                        create_quad_index_buffer_u32(&self.device, vert_length);
                }
            },
            None => {},
        }
    }

    pub fn create_sprite_verts(&mut self, mesh: Mesh<sprite::Vertex>) -> sprite::SpriteVerts {
        self.ensure_sufficient_index_length::<sprite::Vertex>(sprite::VERT_PAGE_SIZE as usize);
        sprite::create_verts_buffer(&self.device, mesh)
    }

    /// Create a new model from the provided mesh.
    /// If the provided mesh is empty this returns None
    pub fn create_model<V: Vertex>(&mut self, mesh: &Mesh<V>) -> Option<Model<V>> {
        self.ensure_sufficient_index_length::<V>(mesh.vertices().len());
        Model::new(&self.device, mesh)
    }

    /// Create a new dynamic model with the specified size.
    pub fn create_dynamic_model<V: Vertex>(&mut self, size: usize) -> DynamicModel<V> {
        DynamicModel::new(&self.device, size)
    }

    /// Update a dynamic model with a mesh and a offset.
    pub fn update_model<V: Vertex>(&self, model: &DynamicModel<V>, mesh: &Mesh<V>, offset: usize) {
        model.update(&self.queue, mesh, offset)
    }

    /// Return the maximum supported texture size.
    pub fn max_texture_size(&self) -> u32 { Self::max_texture_size_raw(&self.device) }

    /// Return the maximum supported texture size from the factory.
    fn max_texture_size_raw(_device: &wgpu::Device) -> u32 {
        // This value is temporary as there are plans to include a way to get this in
        // wgpu this is just a sane standard for now
        8192
    }

    /// Create a new immutable texture from the provided image.
    /// # Panics
    /// If the provided data doesn't completely fill the texture this function
    /// will panic.
    pub fn create_texture_with_data_raw(
        &mut self,
        texture_info: &wgpu::TextureDescriptor,
        view_info: &wgpu::TextureViewDescriptor,
        sampler_info: &wgpu::SamplerDescriptor,
        data: &[u8],
    ) -> Texture {
        let tex = Texture::new_raw(&self.device, texture_info, view_info, sampler_info);

        let size = texture_info.size;
        let block_size = texture_info.format.describe().block_size;
        assert_eq!(
            size.width as usize
                * size.height as usize
                * size.depth_or_array_layers as usize
                * block_size as usize,
            data.len(),
            "Provided data length {} does not fill the provided texture size {:?}",
            data.len(),
            size,
        );

        tex.update(
            &self.queue,
            [0; 2],
            [texture_info.size.width, texture_info.size.height],
            data,
        );

        tex
    }

    /// Create a new raw texture.
    pub fn create_texture_raw(
        &mut self,
        texture_info: &wgpu::TextureDescriptor,
        view_info: &wgpu::TextureViewDescriptor,
        sampler_info: &wgpu::SamplerDescriptor,
    ) -> Texture {
        let texture = Texture::new_raw(&self.device, texture_info, view_info, sampler_info);
        texture.clear(&self.queue); // Needs to be fully initialized for partial writes to work on Dx12 AMD
        texture
    }

    /// Create a new texture from the provided image.
    ///
    /// Currently only supports Rgba8Srgb
    pub fn create_texture(
        &mut self,
        image: &image::DynamicImage,
        filter_method: Option<FilterMode>,
        address_mode: Option<AddressMode>,
    ) -> Result<Texture, RenderError> {
        Texture::new(
            &self.device,
            &self.queue,
            image,
            filter_method,
            address_mode,
        )
    }

    /// Create a new dynamic texture with the
    /// specified dimensions.
    ///
    /// Currently only supports Rgba8Srgb
    pub fn create_dynamic_texture(&mut self, dims: Vec2<u32>) -> Texture {
        Texture::new_dynamic(&self.device, &self.queue, dims.x, dims.y)
    }

    /// Update a texture with the provided offset, size, and data.
    ///
    /// Currently only supports Rgba8Srgb
    pub fn update_texture(
        &mut self,
        texture: &Texture, /* <T> */
        offset: [u32; 2],
        size: [u32; 2],
        // TODO: be generic over pixel type
        data: &[[u8; 4]],
    ) {
        texture.update(&self.queue, offset, size, bytemuck::cast_slice(data))
    }
}

fn create_quad_index_buffer_u16(device: &wgpu::Device, vert_length: usize) -> Buffer<u16> {
    assert!(vert_length <= u16::MAX as usize);
    let indices = [0, 1, 2, 2, 1, 3]
        .iter()
        .cycle()
        .copied()
        .take(vert_length / 4 * 6)
        .enumerate()
        .map(|(i, b)| (i / 6 * 4 + b) as u16)
        .collect::<Vec<_>>();

    Buffer::new(device, wgpu::BufferUsages::INDEX, &indices)
}

fn create_quad_index_buffer_u32(device: &wgpu::Device, vert_length: usize) -> Buffer<u32> {
    assert!(vert_length <= u32::MAX as usize);
    let indices = [0, 1, 2, 2, 1, 3]
        .iter()
        .cycle()
        .copied()
        .take(vert_length / 4 * 6)
        .enumerate()
        .map(|(i, b)| (i / 6 * 4 + b) as u32)
        .collect::<Vec<_>>();

    Buffer::new(device, wgpu::BufferUsages::INDEX, &indices)
}
