#![deny(unsafe_code)]
#![allow(incomplete_features)]
#![allow(clippy::identity_op, clippy::option_map_unit_fn)]
#![deny(clippy::clone_on_ref_ptr)]
#![feature(
    array_methods,
    array_zip,
    bool_to_option,
    drain_filter,
    once_cell,
    trait_alias,
    option_get_or_insert_default
)]
#![recursion_limit = "2048"]

#[macro_use]
pub mod ui;
pub mod audio;
pub mod controller;
mod credits;
mod ecs;
pub mod error;
pub mod game_input;
pub mod hud;
pub mod key_state;
pub mod menu;
pub mod mesh;
pub mod profile;
pub mod render;
pub mod run;
pub mod scene;
pub mod session;
pub mod settings;
pub mod window;

use crate::{
    audio::AudioFrontend,
    profile::Profile,
    render::{Drawer, GlobalsBindGroup},
    settings::Settings,
    window::{Event, Window},
    scene::terrain::SpriteRenderContext,
    settings::{get_fps, AudioOutput},
};
use common::clock::Clock;
use i18n::LocalizationHandle;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// A type used to store state that is shared between all play states.
pub struct GlobalState {
    pub settings: Settings,
    pub profile: Profile,
    pub window: Window,
    pub tokio_runtime: Arc<Runtime>,
    pub lazy_init: scene::terrain::SpriteRenderContextLazy,
    pub audio: AudioFrontend,
    pub info_message: Option<String>,
    pub clock: Clock,
    pub i18n: LocalizationHandle,
    pub clipboard: iced::Clipboard,
    pub client_error: Option<String>,
    pub clear_shadows_next_frame: bool,
}

impl GlobalState {
    pub fn on_play_state_changed(&mut self) {
        self.window.grab_cursor(false);
        self.window.needs_refresh_resize();
    }

    pub fn maintain(&mut self, dt: std::time::Duration) {
        self.audio.maintain(dt);
        self.window.renderer().maintain()
    }

    pub fn paused(&self) -> bool { false }
}

// TODO: appears to be currently unused by playstates
pub enum Direction {
    Forwards,
    Backwards,
}

/// States can either close (and revert to a previous state), push a new state
/// on top of themselves, or switch to a totally different state.
pub enum PlayStateResult {
    /// Keep running this play state.
    Continue,
    /// Pop all play states in reverse order and shut down the program.
    Shutdown,
    /// Close the current play state and pop it from the play state stack.
    Pop,
    /// Push a new play state onto the play state stack.
    Push(Box<dyn PlayState>),
    /// Switch the current play state with a new play state.
    Switch(Box<dyn PlayState>),
}

/// A trait representing a playable game state. This may be a menu, a game
/// session, the title screen, etc.
pub trait PlayState {
    /// Called when entering this play state from another
    fn enter(&mut self, global_state: &mut GlobalState, direction: Direction);

    /// Tick the play state
    fn tick(&mut self, global_state: &mut GlobalState, events: Vec<Event>) -> PlayStateResult;

    /// Get a descriptive name for this state type.
    fn name(&self) -> &'static str;

    /// Determines whether the play state should have an enforced FPS cap
    fn capped_fps(&self) -> bool;

    fn globals_bind_group(&self) -> &GlobalsBindGroup;

    /// Draw the play state.
    fn render<'a>(&'a self, drawer: &mut Drawer<'a>, settings: &Settings);
}



// ----------------------- wasm start ------------------------//
use wasm_bindgen::prelude::*;
use common_assets as res;
pub use wasm_bindgen_rayon::init_thread_pool;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub fn set_resource_data(name: &str, data: &[u8]) {
    res::set_cache_data(name, data);
}

//canvas_id 来自html的canvas
#[wasm_bindgen]
pub fn start() {
    wasm_logger::init(wasm_logger::Config::new(log::Level::Info));
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    log::info!("start wasm");
    //wasm_bindgen_futures::spawn_local(start_game());
    start_game();
}

pub fn start_game() {

    //load setting
    let mut settings = Settings::load();
    settings.display_warnings();

    // TODO: evaluate std::thread::available_concurrency as a num_cpus replacement
    let tokio_runtime = Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap(),
    );

    log::info!("start init audio");
    // Setup audio
    let mut audio = match settings.audio.output {
        AudioOutput::Off => AudioFrontend::no_audio(),
        AudioOutput::Automatic => AudioFrontend::new(settings.audio.num_sfx_channels),
    };

    audio.set_master_volume(settings.audio.master_volume);
    audio.set_music_volume(settings.audio.music_volume);
    audio.set_sfx_volume(settings.audio.sfx_volume);


    // Load the profile.
    let profile = Profile::load();

    //i18n
    let mut i18n =
        LocalizationHandle::load(&settings.language.selected_language).unwrap_or_else(|error| {
            let selected_language = &settings.language.selected_language;
            log::warn!(
                "Impossible to load language: change to the default language (English) instead. {:?} | {:?}",
                error,
                selected_language,
            );
            settings.language.selected_language = i18n::REFERENCE_LANG.to_owned();
            LocalizationHandle::load_expect(&settings.language.selected_language)
        });
    i18n.read().log_missing_entries();
    i18n.set_english_fallback(settings.language.use_english_fallback);
    

    log::info!("start window init");

    //创建运行窗体
    let (mut window, event_loop) = match Window::new(&settings, &tokio_runtime) {
        Ok(ok) => ok,
        Err(error) => panic!("Failed to create window!: {:?}", error),
    };

    log::info!("end window init");
    let clipboard = iced::Clipboard::connect(window.window());
    let lazy_init = SpriteRenderContext::new(window.renderer_mut());
    let global_state = GlobalState {
        audio,
        profile,
        window,
        tokio_runtime,
        lazy_init,
        clock: Clock::new(std::time::Duration::from_secs_f64(1.0 / get_fps(settings.graphics.max_fps) as f64)),
        settings,
        info_message: None,
        i18n,
        clipboard,
        client_error: None,
        clear_shadows_next_frame: false,
    };

    log::info!("start run::run");
    run::run(global_state, event_loop);
}

