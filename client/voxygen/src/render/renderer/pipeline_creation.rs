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
    pub fluid: fluid::FluidPipeline,
    pub lod_terrain: lod_terrain::LodTerrainPipeline,
    pub particle: particle::ParticlePipeline,
    pub clouds: clouds::CloudsPipeline,
    pub bloom: Option<bloom::BloomPipelines>,
    pub postprocess: postprocess::PostProcessPipeline,
    // Consider reenabling at some time
    // player_shadow: figure::FigurePipeline,
    pub skybox: skybox::SkyboxPipeline,
    pub sprite: sprite::SpritePipeline,
    pub terrain: terrain::TerrainPipeline,
    pub ui: ui::UiPipeline,
    pub blit: blit::BlitPipeline,
}

/// Pipelines that are needed to render 3D stuff in-game
/// Use to decouple interface pipeline creation when initializing the renderer
pub struct IngamePipelines {
    debug: debug::DebugPipeline,
    figure: figure::FigurePipeline,
    fluid: fluid::FluidPipeline,
    lod_terrain: lod_terrain::LodTerrainPipeline,
    particle: particle::ParticlePipeline,
    clouds: clouds::CloudsPipeline,
    pub bloom: Option<bloom::BloomPipelines>,
    postprocess: postprocess::PostProcessPipeline,
    // Consider reenabling at some time
    // player_shadow: figure::FigurePipeline,
    skybox: skybox::SkyboxPipeline,
    sprite: sprite::SpritePipeline,
    terrain: terrain::TerrainPipeline,
}

pub struct ShadowPipelines {
    pub point: Option<shadow::PointShadowPipeline>,
    pub directed: Option<shadow::ShadowPipeline>,
    pub figure: Option<shadow::ShadowFigurePipeline>,
}

pub struct IngameAndShadowPipelines {
    pub ingame: IngamePipelines,
    pub shadow: ShadowPipelines,
}

/// Pipelines neccesary to display the UI and take screenshots
/// Use to decouple interface pipeline creation when initializing the renderer
pub struct InterfacePipelines {
    pub ui: ui::UiPipeline,
    pub blit: blit::BlitPipeline,
}

impl Pipelines {
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
            //player_shadow: ingame.player_shadow,
            skybox: ingame.skybox,
            sprite: ingame.sprite,
            terrain: ingame.terrain,
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

        log::warn!("TODO 不支持的 Shader: shaders/point-light-shadows-vert.glsl, 临时用别的Shader代替了");
        let point_light_shadows_vert = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("point_light_shadows_vert"),
            //source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/point-light-shadows-vert.glsl", vert, I "shaders/include/"))),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(include_spirv!("shaders/blit-vert.glsl", vert, I "shaders/include/"))),
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
fn create_interface_pipelines(
    needs: PipelineNeeds,
    pool: &rayon::ThreadPool,
    tasks: [Task; 2],
) -> InterfacePipelines {
    
    let [ui_task, blit_task] = tasks;
    // Construct a pipeline for rendering UI elements
    let create_ui = || {
        ui_task.run(
            || {
                ui::UiPipeline::new(
                    needs.device,
                    &needs.shaders.ui_vert,
                    &needs.shaders.ui_frag,
                    needs.sc_desc,
                    &needs.layouts.global,
                    &needs.layouts.ui,
                )
            },
            "ui pipeline creation",
        )
    };

    // Construct a pipeline for blitting, used during screenshotting
    let create_blit = || {
        blit_task.run(
            || {
                blit::BlitPipeline::new(
                    needs.device,
                    &needs.shaders.blit_vert,
                    &needs.shaders.blit_frag,
                    needs.sc_desc,
                    &needs.layouts.blit,
                )
            },
            "blit pipeline creation",
        )
    };

    let (ui, blit) = pool.join(create_ui, create_blit);

    InterfacePipelines { ui, blit }
}

/// Create IngamePipelines and shadow pipelines in parallel
fn create_ingame_and_shadow_pipelines(
    needs: PipelineNeeds,
    pool: &rayon::ThreadPool,
    tasks: [Task; 14],
) -> IngameAndShadowPipelines {
    
    let PipelineNeeds {
        device,
        layouts,
        shaders,
        pipeline_modes,
        sc_desc,
    } = needs;

    let [
        debug_task,
        skybox_task,
        figure_task,
        terrain_task,
        fluid_task,
        sprite_task,
        particle_task,
        lod_terrain_task,
        clouds_task,
        bloom_task,
        postprocess_task,
        // TODO: if these are ever actually optionally done, counting them
        // as tasks to do beforehand seems kind of iffy since they will just
        // be skipped
        point_shadow_task,
        terrain_directed_shadow_task,
        figure_directed_shadow_task,
    ] = tasks;

    // TODO: pass in format of target color buffer

    // Pipeline for rendering debug shapes
    let create_debug = || {
        debug_task.run(
            || {
                debug::DebugPipeline::new(
                    device,
                    &shaders.debug_vert,
                    &shaders.debug_frag,
                    &layouts.global,
                    &layouts.debug,
                    pipeline_modes.aa,
                )
            },
            "debug pipeline creation",
        )
    };
    // Pipeline for rendering skyboxes
    let create_skybox = || {
        skybox_task.run(
            || {
                skybox::SkyboxPipeline::new(
                    device,
                    &shaders.skybox_vert,
                    &shaders.skybox_frag,
                    &layouts.global,
                    pipeline_modes.aa,
                )
            },
            "skybox pipeline creation",
        )
    };
    // Pipeline for rendering figures
    let create_figure = || {
        figure_task.run(
            || {
                figure::FigurePipeline::new(
                    device,
                    &shaders.figure_vert,
                    &shaders.figure_frag,
                    &layouts.global,
                    &layouts.figure,
                    pipeline_modes.aa,
                )
            },
            "figure pipeline creation",
        )
    };
    // Pipeline for rendering terrain
    let create_terrain = || {
        terrain_task.run(
            || {
                terrain::TerrainPipeline::new(
                    device,
                    &shaders.terrain_vert,
                    &shaders.terrain_frag,
                    &layouts.global,
                    &layouts.terrain,
                    pipeline_modes.aa,
                )
            },
            "terrain pipeline creation",
        )
    };
    // Pipeline for rendering fluids
    let create_fluid = || {
        fluid_task.run(
            || {
                fluid::FluidPipeline::new(
                    device,
                    &shaders.fluid_vert,
                    &shaders.fluid_frag,
                    &layouts.global,
                    &layouts.terrain,
                    pipeline_modes.aa,
                )
            },
            "fluid pipeline creation",
        )
    };
    // Pipeline for rendering sprites
    let create_sprite = || {
        sprite_task.run(
            || {
                sprite::SpritePipeline::new(
                    device,
                    &shaders.sprite_vert,
                    &shaders.sprite_frag,
                    &layouts.global,
                    &layouts.sprite,
                    &layouts.terrain,
                    pipeline_modes.aa,
                )
            },
            "sprite pipeline creation",
        )
    };
    // Pipeline for rendering particles
    let create_particle = || {
        particle_task.run(
            || {
                particle::ParticlePipeline::new(
                    device,
                    &shaders.particle_vert,
                    &shaders.particle_frag,
                    &layouts.global,
                    pipeline_modes.aa,
                )
            },
            "particle pipeline creation",
        )
    };
    // Pipeline for rendering terrain
    let create_lod_terrain = || {
        lod_terrain_task.run(
            || {
                lod_terrain::LodTerrainPipeline::new(
                    device,
                    &shaders.lod_terrain_vert,
                    &shaders.lod_terrain_frag,
                    &layouts.global,
                    pipeline_modes.aa,
                )
            },
            "lod terrain pipeline creation",
        )
    };
    // Pipeline for rendering our clouds (a kind of post-processing)
    let create_clouds = || {
        clouds_task.run(
            || {
                clouds::CloudsPipeline::new(
                    device,
                    &shaders.clouds_vert,
                    &shaders.clouds_frag,
                    &layouts.global,
                    &layouts.clouds,
                    pipeline_modes.aa,
                )
            },
            "clouds pipeline creation",
        )
    };
    // Pipelines for rendering our bloom
    let create_bloom = || {
        bloom_task.run(
            || {
                match &pipeline_modes.bloom {
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
                })
            },
            "bloom pipelines creation",
        )
    };
    // Pipeline for rendering our post-processing
    let create_postprocess = || {
        postprocess_task.run(
            || {
                postprocess::PostProcessPipeline::new(
                    device,
                    &shaders.postprocess_vert,
                    &shaders.postprocess_frag,
                    sc_desc,
                    &layouts.global,
                    &layouts.postprocess,
                )
            },
            "postprocess pipeline creation",
        )
    };

    // Pipeline for rendering point light terrain shadow maps.
    let create_point_shadow = || {
        point_shadow_task.run(
            || {
                shadow::PointShadowPipeline::new(
                    device,
                    &shaders.point_light_shadows_vert,
                    &layouts.global,
                    &layouts.terrain,
                    pipeline_modes.aa,
                )
            },
            "point shadow pipeline creation",
        )
    };
    // Pipeline for rendering directional light terrain shadow maps.
    let create_terrain_directed_shadow = || {
        terrain_directed_shadow_task.run(
            || {
                shadow::ShadowPipeline::new(
                    device,
                    &shaders.light_shadows_directed_vert,
                    &layouts.global,
                    &layouts.terrain,
                    pipeline_modes.aa,
                )
            },
            "terrain directed shadow pipeline creation",
        )
    };
    // Pipeline for rendering directional light figure shadow maps.
    let create_figure_directed_shadow = || {
        figure_directed_shadow_task.run(
            || {
                shadow::ShadowFigurePipeline::new(
                    device,
                    &shaders.light_shadows_figure_vert,
                    &layouts.global,
                    &layouts.figure,
                    pipeline_modes.aa,
                )
            },
            "figure directed shadow pipeline creation",
        )
    };

    let j1 = || pool.join(create_debug, || pool.join(create_skybox, create_figure));
    let j2 = || pool.join(create_terrain, || pool.join(create_fluid, create_bloom));
    let j3 = || pool.join(create_sprite, create_particle);
    let j4 = || pool.join(create_lod_terrain, create_clouds);
    let j5 = || pool.join(create_postprocess, create_point_shadow);
    let j6 = || {
        pool.join(
            create_terrain_directed_shadow,
            create_figure_directed_shadow,
        )
    };

    // Ignore this
    let (
        (
            ((debug, (skybox, figure)), (terrain, (fluid, bloom))),
            ((sprite, particle), (lod_terrain, clouds)),
        ),
        ((postprocess, point_shadow), (terrain_directed_shadow, figure_directed_shadow)),
    ) = pool.join(
        || pool.join(|| pool.join(j1, j2), || pool.join(j3, j4)),
        || pool.join(j5, j6),
    );

    IngameAndShadowPipelines {
        ingame: IngamePipelines {
            debug,
            figure,
            fluid,
            lod_terrain,
            particle,
            clouds,
            bloom,
            postprocess,
            skybox,
            sprite,
            terrain,
            // player_shadow_pipeline,
        },
        // TODO: skip creating these if the shadow map setting is not enabled
        shadow: ShadowPipelines {
            point: Some(point_shadow),
            directed: Some(terrain_directed_shadow),
            figure: Some(figure_directed_shadow),
        },
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
    has_shadow_views: bool,
) -> Result<
    (
        InterfacePipelines,
        PipelineCreation<IngameAndShadowPipelines>,
    ),
    RenderError,
> {
    // Process shaders into modules
    let shader_modules = ShaderModules::new(&device)?;

    // Create threadpool for parallel portion
    let pool = rayon::ThreadPoolBuilder::new()
        .thread_name(|n| format!("pipeline-creation-{}", n))
        .build()
        .unwrap();

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
    let interface_pipelines =
        create_interface_pipelines(needs, &pool, Progress::new().create_tasks());

    let pool = Arc::new(pool);
    let send_pool = Arc::clone(&pool);
    // Track pipeline creation progress
    let progress = Arc::new(Progress::new());
    let (pipeline_send, pipeline_recv) = crossbeam_channel::bounded(0);
    let pipeline_creation = PipelineCreation {
        progress: Arc::clone(&progress),
        recv: pipeline_recv,
    };
    // Start background compilation
    pool.spawn(move || {
        let pool = &*send_pool;

        let needs = PipelineNeeds {
            device: &device,
            layouts: &layouts,
            shaders: &shader_modules,
            pipeline_modes: &pipeline_modes,
            sc_desc: &sc_desc,
        };

        let pipelines = create_ingame_and_shadow_pipelines(needs, pool, progress.create_tasks());

        pipeline_send.send(pipelines).expect("Channel disconnected");
    });

    Ok((interface_pipelines, pipeline_creation))
}

/// Creates all the pipelines used to render.
/// Use this to recreate all the pipelines in the background.
/// TODO: report progress
/// NOTE: this tries to use all the CPU cores to complete as soon as possible
pub(super) fn recreate_pipelines(
    device: Arc<wgpu::Device>,
    immutable_layouts: Arc<ImmutableLayouts>,
    pipeline_modes: PipelineModes,
    sc_desc: wgpu::SurfaceConfiguration,
    has_shadow_views: bool,
) -> PipelineCreation<
    Result<
        (
            Pipelines,
            ShadowPipelines,
            Arc<postprocess::PostProcessLayout>,
        ),
        RenderError,
    >,
> {
    // Create threadpool for parallel portion
    let pool = rayon::ThreadPoolBuilder::new()
        .thread_name(|n| format!("pipeline-recreation-{}", n))
        .build()
        .unwrap();
    let pool = Arc::new(pool);
    let send_pool = Arc::clone(&pool);
    // Track pipeline creation progress
    let progress = Arc::new(Progress::new());
    let (result_send, result_recv) = crossbeam_channel::bounded(0);
    let pipeline_creation = PipelineCreation {
        progress: Arc::clone(&progress),
        recv: result_recv,
    };
    // Start background compilation
    pool.spawn(move || {
        let pool = &*send_pool;

        // Create tasks upfront so the total counter will be accurate
        let shader_task = progress.create_task();
        let interface_tasks = progress.create_tasks();
        let ingame_and_shadow_tasks = progress.create_tasks();

        // Process shaders into modules
        let guard = shader_task.start("process shaders");
        let shader_modules =
            match ShaderModules::new(&device) {
                Ok(modules) => modules,
                Err(err) => {
                    result_send.send(Err(err)).expect("Channel disconnected");
                    return;
                },
            };
        drop(guard);

        // Create new postprocess layouts
        let postprocess_layouts = Arc::new(postprocess::PostProcessLayout::new(
            &device,
            &pipeline_modes,
        ));

        let layouts = Layouts {
            immutable: immutable_layouts,
            postprocess: postprocess_layouts,
        };

        let needs = PipelineNeeds {
            device: &device,
            layouts: &layouts,
            shaders: &shader_modules,
            pipeline_modes: &pipeline_modes,
            sc_desc: &sc_desc,
        };

        // Create interface pipelines
        let interface = create_interface_pipelines(needs, pool, interface_tasks);

        // Create the rest of the pipelines
        let IngameAndShadowPipelines { ingame, shadow } =
            create_ingame_and_shadow_pipelines(needs, pool, ingame_and_shadow_tasks);

        // Send them
        result_send
            .send(Ok((
                Pipelines::consolidate(interface, ingame),
                shadow,
                layouts.postprocess,
            )))
            .expect("Channel disconnected");
    });

    pipeline_creation
}

use core::sync::atomic::{AtomicUsize, Ordering};

/// Represents future task that has not been started
/// Dropping this will mark the task as complete though
struct Task<'a> {
    progress: &'a Progress,
}

/// Represents in-progress task, drop when complete
// NOTE: fields are unused because they are only used for their Drop impls
struct StartedTask<'a> {
    _task: Task<'a>,
}

#[derive(Default)]
struct Progress {
    total: AtomicUsize,
    complete: AtomicUsize,
    // Note: could easily add a "started counter" if that would be useful
}

impl Progress {
    pub fn new() -> Self { Self::default() }

    /// Creates a task incrementing the total number of tasks
    /// NOTE: all tasks should be created as upfront as possible so that the
    /// total reflects the amount of tasks that will need to be completed
    pub fn create_task(&self) -> Task {
        self.total.fetch_add(1, Ordering::Relaxed);
        Task { progress: self }
    }

    /// Helper method for creating tasks to do in bulk
    pub fn create_tasks<const N: usize>(&self) -> [Task; N] { [(); N].map(|()| self.create_task()) }
}

impl<'a> Task<'a> {
    /// Start a task.
    /// The name is used for profiling.
    fn start(self, _name: &str) -> StartedTask<'a> {
        // _name only used when tracy feature is activated
        StartedTask {
            _task: self,
        }
    }

    /// Convenience function to run the provided closure as the task
    /// Completing the task when this function returns
    fn run<T>(self, task: impl FnOnce() -> T, name: &str) -> T {
        let _guard = self.start(name);
        task()
    }
}

impl Drop for Task<'_> {
    fn drop(&mut self) { self.progress.complete.fetch_add(1, Ordering::Relaxed); }
}

pub struct PipelineCreation<T> {
    progress: Arc<Progress>,
    recv: crossbeam_channel::Receiver<T>,
}

impl<T> PipelineCreation<T> {
    /// Returns the number of pipelines being built and completed
    /// (total, complete)
    /// NOTE: there is no guarantee that `total >= complete` due to relaxed
    /// atomics but this property should hold most of the time
    pub fn status(&self) -> (usize, usize) {
        let progress = &*self.progress;
        (
            progress.total.load(Ordering::Relaxed),
            progress.complete.load(Ordering::Relaxed),
        )
    }

    /// Checks if the pipelines were completed and returns the result if they
    /// were
    pub fn try_complete(self) -> Result<T, Self> {
        use crossbeam_channel::TryRecvError;
        match self.recv.try_recv() {
            // Yay!
            Ok(t) => Ok(t),
            // Normal error, we have not gotten anything yet
            Err(TryRecvError::Empty) => Err(self),
            // How rude!
            Err(TryRecvError::Disconnected) => {
                panic!(
                    "Background thread panicked or dropped the sender without sending anything!"
                );
            },
        }
    }
}
