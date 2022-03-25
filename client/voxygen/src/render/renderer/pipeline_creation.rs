use super::{
    super::{
        pipelines::{
            blit, bloom, clouds, debug, figure, fluid, lod_terrain, particle, postprocess, shadow,
            skybox, sprite, terrain, ui,
        },
        BloomMode, PipelineModes, RenderError,
        //AaMode, ShadowMode, CloudMode, FluidMode, LightingMode, 
    },
    ImmutableLayouts, Layouts,
};
use std::sync::Arc;

/// All the pipelines
pub struct Pipelines {
    pub debug: debug::DebugPipeline,
    pub figure: figure::FigurePipeline,

    #[cfg(not(target_arch = "wasm32"))]
    pub fluid: fluid::FluidPipeline,

    #[cfg(not(target_arch = "wasm32"))]
    pub lod_terrain: lod_terrain::LodTerrainPipeline,

    #[cfg(not(target_arch = "wasm32"))]
    pub particle: particle::ParticlePipeline,

    pub clouds: clouds::CloudsPipeline,
    pub bloom: Option<bloom::BloomPipelines>,
    pub postprocess: postprocess::PostProcessPipeline,
    pub skybox: skybox::SkyboxPipeline,

    #[cfg(not(target_arch = "wasm32"))]
    pub sprite: sprite::SpritePipeline,

    #[cfg(not(target_arch = "wasm32"))]
    pub terrain: terrain::TerrainPipeline,
    
    pub ui: ui::UiPipeline,
    pub blit: blit::BlitPipeline,
}

/// Pipelines that are needed to render 3D stuff in-game
/// Use to decouple interface pipeline creation when initializing the renderer
pub struct IngamePipelines {
    debug: debug::DebugPipeline,
    figure: figure::FigurePipeline,

    #[cfg(not(target_arch = "wasm32"))]
    fluid: fluid::FluidPipeline,

    #[cfg(not(target_arch = "wasm32"))]
    lod_terrain: lod_terrain::LodTerrainPipeline,

    #[cfg(not(target_arch = "wasm32"))]
    particle: particle::ParticlePipeline,


    clouds: clouds::CloudsPipeline,
    pub bloom: Option<bloom::BloomPipelines>,
    postprocess: postprocess::PostProcessPipeline,
    skybox: skybox::SkyboxPipeline,

    #[cfg(not(target_arch = "wasm32"))]
    sprite: sprite::SpritePipeline,

    #[cfg(not(target_arch = "wasm32"))]
    terrain: terrain::TerrainPipeline,
}

pub struct ShadowPipelines {
    pub point: Option<shadow::PointShadowPipeline>,
    pub directed: Option<shadow::ShadowPipeline>,
    pub figure: Option<shadow::ShadowFigurePipeline>,

}
pub struct InterfacePipelines {
    pub ui: ui::UiPipeline,
    pub blit: blit::BlitPipeline,
}

impl Pipelines {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn consolidate(interface: InterfacePipelines, ingame: IngamePipelines) -> Self {
        Self {
            debug: ingame.debug,
            figure: ingame.figure,
            fluid: ingame.fluid,
            lod_terrain: ingame.lod_terrain,
            particle: ingame.particle,
            clouds: ingame.clouds,
            bloom: ingame.bloom,
            postprocess: ingame.postprocess,
            skybox: ingame.skybox,
            sprite: ingame.sprite,
            terrain: ingame.terrain,
            ui: interface.ui,
            blit: interface.blit,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn consolidate(interface: InterfacePipelines, ingame: IngamePipelines) -> Self {
        Self {
            debug: ingame.debug,
            figure: ingame.figure,
            //fluid: ingame.fluid,
            //lod_terrain: ingame.lod_terrain,
            //particle: ingame.particle,
            clouds: ingame.clouds,
            bloom: ingame.bloom,
            postprocess: ingame.postprocess,
            skybox: ingame.skybox,
            //sprite: ingame.sprite,
            //terrain: ingame.terrain,
            ui: interface.ui,
            blit: interface.blit,
        }
    }
}

/// Processed shaders ready for use in pipeline creation
struct ShaderModules {
    skybox_vert: wgpu::ShaderModule,
    skybox_frag: wgpu::ShaderModule,
    debug_vert: wgpu::ShaderModule,
    debug_frag: wgpu::ShaderModule,
    figure_vert: wgpu::ShaderModule,
    figure_frag: wgpu::ShaderModule,
    terrain_vert: wgpu::ShaderModule,
    terrain_frag: wgpu::ShaderModule,
    fluid_vert: wgpu::ShaderModule,
    fluid_frag: wgpu::ShaderModule,
    sprite_vert: wgpu::ShaderModule,
    sprite_frag: wgpu::ShaderModule,
    particle_vert: wgpu::ShaderModule,
    particle_frag: wgpu::ShaderModule,
    ui_vert: wgpu::ShaderModule,
    ui_frag: wgpu::ShaderModule,
    lod_terrain_vert: wgpu::ShaderModule,
    lod_terrain_frag: wgpu::ShaderModule,
    clouds_vert: wgpu::ShaderModule,
    clouds_frag: wgpu::ShaderModule,
    dual_downsample_filtered_frag: wgpu::ShaderModule,
    dual_downsample_frag: wgpu::ShaderModule,
    dual_upsample_frag: wgpu::ShaderModule,
    postprocess_vert: wgpu::ShaderModule,
    postprocess_frag: wgpu::ShaderModule,
    blit_vert: wgpu::ShaderModule,
    blit_frag: wgpu::ShaderModule,

    #[cfg(not(target_arch = "wasm32"))]
    point_light_shadows_vert: wgpu::ShaderModule,
    
    light_shadows_directed_vert: wgpu::ShaderModule,
    light_shadows_figure_vert: wgpu::ShaderModule,
}

impl ShaderModules {
    pub fn new(device: &wgpu::Device) -> Result<Self, RenderError> {

        //配置改为在shader中写死
        //  contrant:
        //      ...
        //
        //  cloud：
        //      #include <cloud_regular.glsl> 
        //               <cloud_none.glsl>
        //
        //  FluidMode 为加载不同shader 
        //      shaders/fluid-frag/shiny.glsl
        //      shaders/fluid-frag/cheap.glsl
        //
        //  anti_alias:
        //      #include <antialias-fxaa.glsl> 
        //               <antialias-msaa-x4.glsl> 
        //               <antialias-msaa-x8.glsl> 
        //               <antialias-<msaa-x16.glsl> 
        //               <antialias-none.glsl>


        use inline_spirv::include_spirv;
        use std::borrow::Cow;
        let skybox_vert = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("skybox_vert"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/skybox-vert.glsl", vert, I "shaders/include/"))),
        });

        let skybox_frag = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("skybox_frag"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/skybox-frag.glsl", frag, I "shaders/include/"))),
        });

        let debug_vert = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("debug_vert"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/debug-vert.glsl", vert, I "shaders/include/"))),
        });

        let debug_frag = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("debug_frag"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/debug-frag.glsl", frag, I "shaders/include/"))),
        });

        let figure_vert = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("figure_vert"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/figure-vert.glsl", vert, I "shaders/include/"))),
        });

        let figure_frag = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("figure_frag"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/figure-frag.glsl", frag, I "shaders/include/"))),
        });

        let terrain_vert = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("terrain_vert"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/terrain-vert.glsl", vert, I "shaders/include/"))),
        });

        let terrain_frag = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("terrain_frag"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/terrain-frag.glsl", frag, I "shaders/include/"))),
        });

        let fluid_vert = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("fluid_vert"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/fluid-vert.glsl", vert, I "shaders/include/"))),
        });

        let fluid_frag = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("fluid_frag"),
            //todo use shiny? cheap
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/fluid-frag/shiny.glsl", frag, I "shaders/include/"))),
        });

        let sprite_vert = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("sprite_vert"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/sprite-vert.glsl", vert, I "shaders/include/"))),
        });

        let sprite_frag = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("sprite_frag"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/sprite-frag.glsl", frag, I "shaders/include/"))),
        });

        let particle_vert = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("particle_vert"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/particle-vert.glsl", vert, I "shaders/include/"))),
        });

        let particle_frag = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("particle_frag"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/particle-frag.glsl", frag, I "shaders/include/"))),
        });

        let ui_vert = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("ui_vert"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/ui-vert.glsl", vert, I "shaders/include/"))),
        });

        let ui_frag = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("ui_frag"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/ui-frag.glsl", frag, I "shaders/include/"))),
        });

        let lod_terrain_vert = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("lod_terrain_vert"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/lod-terrain-vert.glsl", vert, I "shaders/include/"))),
        });

        let lod_terrain_frag = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("lod_terrain_frag"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/lod-terrain-frag.glsl", frag, I "shaders/include/"))),
        });

        let clouds_vert = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("clouds_vert"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/clouds-vert.glsl", vert, I "shaders/include/"))),
        });

        let clouds_frag = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("clouds_frag"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/clouds-frag.glsl", frag, I "shaders/include/", I "shaders/antialias/"))),
        });

        let dual_downsample_filtered_frag = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("dual_downsample_filtered_frag"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/dual-downsample-filtered-frag.glsl", frag, I "shaders/include/"))),
        });

        let dual_downsample_frag = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("dual_downsample_frag"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/dual-downsample-frag.glsl", frag, I "shaders/include/"))),
        });

        let dual_upsample_frag = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("dual_upsample_frag"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/dual-upsample-frag.glsl", frag, I "shaders/include/"))),
        });

        let postprocess_vert = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("postprocess_vert"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/postprocess-vert.glsl", vert, I "shaders/include/"))),
        });

        let postprocess_frag = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("postprocess_frag"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/postprocess-frag.glsl", frag, I "shaders/include/", I "shaders/antialias/"))),
        });

        let blit_vert = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("blit_vert"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/blit-vert.glsl", vert, I "shaders/include/"))),
        });

        let blit_frag = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("blit_frag"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/blit-frag.glsl", frag, I "shaders/include/"))),
        });

        log::warn!("TODO 不支持的 Shader: shaders/point-light-shadows-vert.glsl, 暂时屏蔽");

        #[cfg(not(target_arch = "wasm32"))]
        let point_light_shadows_vert = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("point_light_shadows_vert"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/point-light-shadows-vert.glsl", vert, I "shaders/include/"))),
        });

        let light_shadows_directed_vert = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("light_shadows_directed_vert"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/light-shadows-directed-vert.glsl", vert, I "shaders/include/"))),
        });

        let light_shadows_figure_vert = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("light_shadows_figure_vert"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/light-shadows-figure-vert.glsl", vert, I "shaders/include/"))),
        });

        Ok(Self {
            skybox_vert, 
            skybox_frag, 
            debug_vert, 
            debug_frag, 
            figure_vert, 
            figure_frag,
            terrain_vert, 
            terrain_frag, 
            fluid_vert, 
            fluid_frag, 
            sprite_vert, 
            sprite_frag,
            particle_vert,
            particle_frag, 
            ui_vert, 
            ui_frag, 
            lod_terrain_vert, 
            lod_terrain_frag, 
            clouds_vert, 
            clouds_frag,
            dual_downsample_filtered_frag,
            dual_downsample_frag,
            dual_upsample_frag,
            postprocess_vert,
            postprocess_frag,
            blit_vert,
            blit_frag,

            #[cfg(not(target_arch = "wasm32"))]
            point_light_shadows_vert,

            light_shadows_directed_vert,
            light_shadows_figure_vert,
        })
    }
}

/// Things needed to create a pipeline
#[derive(Clone, Copy)]
struct PipelineNeeds<'a> {
    device: &'a wgpu::Device,
    layouts: &'a Layouts,
    shaders: &'a ShaderModules,
    pipeline_modes: &'a PipelineModes,
    sc_desc: &'a wgpu::SurfaceConfiguration,
}

/// Creates InterfacePipelines in parallel
fn create_interface_pipelines(needs: PipelineNeeds) -> InterfacePipelines {

    let ui = ui::UiPipeline::new(
        needs.device,
        &needs.shaders.ui_vert,
        &needs.shaders.ui_frag,
        needs.sc_desc,
        &needs.layouts.global,
        &needs.layouts.ui,
    );

    let blit =  blit::BlitPipeline::new(
        needs.device,
        &needs.shaders.blit_vert,
        &needs.shaders.blit_frag,
        needs.sc_desc,
        &needs.layouts.blit,
    );

    InterfacePipelines { ui, blit }
}

fn create_shadow_pipelines(needs: PipelineNeeds) -> ShadowPipelines {

    // shader不支持，屏蔽管线
    log::warn!("Wasm 不支持的Pipeline(Shader Error): PointShadowPipeline");
    #[cfg(not(target_arch = "wasm32"))]
    let point_shadow = shadow::PointShadowPipeline::new(
        needs.device,
        &needs.shaders.point_light_shadows_vert,
        &needs.layouts.global,
        &needs.layouts.terrain,
        needs.pipeline_modes.aa,
    );
   
    log::warn!("Wasm 不支持的Pipeline(Shader Error): ShadowPipeline");
    #[cfg(not(target_arch = "wasm32"))]
    let terrain_directed_shadow = shadow::ShadowPipeline::new(
        needs.device,
        &needs.shaders.light_shadows_directed_vert,
        &needs.layouts.global,
        &needs.layouts.terrain,
        needs.pipeline_modes.aa,
    );

    log::warn!("Wasm 不支持的Pipeline(Shader Error): ShadowFigurePipeline");
    #[cfg(not(target_arch = "wasm32"))]
    let figure_directed_shadow = shadow::ShadowFigurePipeline::new(
        needs.device,
        &needs.shaders.light_shadows_figure_vert,
        &needs.layouts.global,
        &needs.layouts.figure,
        needs.pipeline_modes.aa,
    );

    
    ShadowPipelines {
        #[cfg(not(target_arch = "wasm32"))]
        point: Some(point_shadow),
        #[cfg(target_arch = "wasm32")]
        point: None,

        #[cfg(not(target_arch = "wasm32"))]
        directed: Some(terrain_directed_shadow),
        #[cfg(target_arch = "wasm32")]
        directed: None,

        #[cfg(not(target_arch = "wasm32"))]
        figure: Some(figure_directed_shadow),
        #[cfg(target_arch = "wasm32")]
        figure: None,
    }
}

/// Create IngamePipelines and shadow pipelines in parallel
fn create_ingame_pipelines(needs: PipelineNeeds) -> IngamePipelines {

    let PipelineNeeds {
        device,
        layouts,
        shaders,
        pipeline_modes,
        sc_desc,
    } = needs;

    let debug = debug::DebugPipeline::new(
        device,
        &shaders.debug_vert,
        &shaders.debug_frag,
        &layouts.global,
        &layouts.debug,
        pipeline_modes.aa,
    );

    let skybox = skybox::SkyboxPipeline::new(
        device,
        &shaders.skybox_vert,
        &shaders.skybox_frag,
        &layouts.global,
        pipeline_modes.aa,
    );

    let figure = figure::FigurePipeline::new(
        device,
        &shaders.figure_vert,
        &shaders.figure_frag,
        &layouts.global,
        &layouts.figure,
        pipeline_modes.aa,
    );

    log::warn!("Wasm 不支持的Pipeline(Shader Error): TerrainPipeline");
    #[cfg(not(target_arch = "wasm32"))]
    let terrain =  terrain::TerrainPipeline::new(
        device,
        &shaders.terrain_vert,
        &shaders.terrain_frag,
        &layouts.global,
        &layouts.terrain,
        pipeline_modes.aa,
    );

    log::warn!("Wasm 不支持的Pipeline(Shader Error): FluidPipeline");
    #[cfg(not(target_arch = "wasm32"))]
    let fluid = fluid::FluidPipeline::new(
        device,
        &shaders.fluid_vert,
        &shaders.fluid_frag,
        &layouts.global,
        &layouts.terrain,
        pipeline_modes.aa,
    );

    log::warn!("Wasm 不支持的Pipeline(Shader Error): SpritePipeline");
    #[cfg(not(target_arch = "wasm32"))]
    let sprite =  sprite::SpritePipeline::new(
        device,
        &shaders.sprite_vert,
        &shaders.sprite_frag,
        &layouts.global,
        &layouts.sprite,
        &layouts.terrain,
        pipeline_modes.aa,
    );

    log::warn!("Wasm 不支持的Pipeline(Shader Error): ParticlePipeline");
    #[cfg(not(target_arch = "wasm32"))]
    let particle = particle::ParticlePipeline::new(
        device,
        &shaders.particle_vert,
        &shaders.particle_frag,
        &layouts.global,
        pipeline_modes.aa,
    );

    log::warn!("不支持的Pipeline(Shader Error): LodTerrainPipeline");
    #[cfg(not(target_arch = "wasm32"))]
    let lod_terrain = lod_terrain::LodTerrainPipeline::new(
        device,
        &shaders.lod_terrain_vert,
        &shaders.lod_terrain_frag,
        &layouts.global,
        pipeline_modes.aa,
    );

   
    let clouds =  clouds::CloudsPipeline::new(
        device,
        &shaders.clouds_vert,
        &shaders.clouds_frag,
        &layouts.global,
        &layouts.clouds,
        pipeline_modes.aa,
    );
    
    let bloom = match &pipeline_modes.bloom {
        BloomMode::Off => None,
        BloomMode::On(config) => Some(config),
    }
    .map(|bloom_config| {
        bloom::BloomPipelines::new(
            device,
            &shaders.blit_vert,
            &shaders.dual_downsample_filtered_frag,
            &shaders.dual_downsample_frag,
            &shaders.dual_upsample_frag,
            wgpu::TextureFormat::Rgba16Float,
            &layouts.bloom,
            bloom_config,
        )
    });

    let postprocess = postprocess::PostProcessPipeline::new(
        device,
        &shaders.postprocess_vert,
        &shaders.postprocess_frag,
        sc_desc,
        &layouts.global,
        &layouts.postprocess,
    );


   
    IngamePipelines {
        debug,
        figure,
        
        #[cfg(not(target_arch = "wasm32"))]
        fluid,

        #[cfg(not(target_arch = "wasm32"))]
        lod_terrain,

        #[cfg(not(target_arch = "wasm32"))]
        particle,


        clouds,
        bloom,
        postprocess,
        skybox,

        #[cfg(not(target_arch = "wasm32"))]
        sprite,

        #[cfg(not(target_arch = "wasm32"))]
        terrain,
    }
}

/// Creates all the pipelines used to render.
/// Use this for the initial creation.
/// It blocks the main thread to create the interface pipelines while moving the
/// creation of other pipelines into the background
/// NOTE: this tries to use all the CPU cores to complete as soon as possible
pub(super) fn initial_create_pipelines(
    device: Arc<wgpu::Device>,
    layouts: Layouts,
    pipeline_modes: PipelineModes,
    sc_desc: wgpu::SurfaceConfiguration,
) -> Result<
    (
        InterfacePipelines,
        IngamePipelines,
        ShadowPipelines
    ),
    RenderError,
> {
    // Process shaders into modules
    let shader_modules = ShaderModules::new(&device)?;

    let needs = PipelineNeeds {
        device: &device,
        layouts: &layouts,
        shaders: &shader_modules,
        pipeline_modes: &pipeline_modes,
        sc_desc: &sc_desc,
    };

    // Create interface pipelines while blocking the main thread
    // Note: we use a throwaway Progress tracker here since we don't need to track
    // the progress
    let interface_pipelines = create_interface_pipelines(needs);
    let ingame_pipelines = create_ingame_pipelines(needs);
    let shadow_pipelines = create_shadow_pipelines(needs);

    Ok((interface_pipelines, ingame_pipelines, shadow_pipelines))
}
