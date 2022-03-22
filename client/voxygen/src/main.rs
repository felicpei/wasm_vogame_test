#![deny(unsafe_code)]
#![feature(bool_to_option)]
#![recursion_limit = "2048"]

fn main() {

    //init log
    let mut builder = env_logger::Builder::new();
    builder.filter_module("wgpu", log::LevelFilter::Warn);
    builder.filter_module("wgpu_core", log::LevelFilter::Warn);
    builder.filter_level(log::LevelFilter::Info);
    builder.init();

    log::info!("inited log");

    //放到lib里，和wasm通用
    veloren_voxygen::start_game();
}
