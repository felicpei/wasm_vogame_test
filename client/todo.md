
## todo
- 目前网络部分使用了多线程，这个wasm会有问题，修改 tokio 的 rt-multi-thread 为 rt，wasm不支持 rt-multi-thread。
- 网络部分暂时屏蔽了tcpsocket，以后换。
- 加载不支持loaddir的问题，目前i18n使用
- 不支持的Shader
  - point-light-shadows-vert.glsl
  - 引用 shadows.glsl 的shader
- shader不支持的渲染管线
  - PointShadowPipeline
  - ShadowPipeline
  - ShadowFigurePipeline
  - TerrainPipeline
  - FluidPipeline
  - SpritePipeline
  - ParticlePipeline
  - LodTerrainPipeline
- 多线程问题
  - 目前wasm虽然支持多线程，但是线程必须从js开出来，无法从rust代码开，这样会导致以前所有的多线程代码无法运行，只能先改为单线程
  - wasm线程方式为：js开出的线程然后调用wasm的接口来实现多线程，目前几乎无法使用。
    - pipeline_creation.rs 渲染管线改为单线程（效率未知）
    - terrain.rs SpriteRenderContext 改为单线程

