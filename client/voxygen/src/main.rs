#![deny(unsafe_code)]
#![feature(bool_to_option)]
#![recursion_limit = "2048"]

use i18n::{self, LocalizationHandle};
use veloren_voxygen::{
    audio::AudioFrontend,
    profile::Profile,
    run,
    scene::terrain::SpriteRenderContext,
    settings::{get_fps, AudioOutput, Settings},
    window::Window,
    GlobalState,
};

use common::clock::Clock;
use std::panic;

fn main() {

    //init log
    init_log ();

    // Load the settings
    // Note: This won't log anything due to it being called before
    // `logging::init`. The issue is we need to read a setting to decide
    // whether we create a log file or not.
    let mut settings = Settings::load();
    settings.display_warnings();

    // Set up panic handler to relay swish panic messages to the user
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let panic_info_payload = panic_info.payload();
        let payload_string = panic_info_payload.downcast_ref::<String>();
        let reason = match payload_string {
            Some(s) => s,
            None => {
                let payload_str = panic_info_payload.downcast_ref::<&str>();
                match payload_str {
                    Some(st) => st,
                    None => "Payload is not a string",
                }
            },
        };

        log::error!(
            "VOXYGEN HAS PANICKED\n{:?}",
            reason,
        );

        default_hook(panic_info);
    }));

    // Setup tokio runtime
    use common::consts::MIN_RECOMMENDED_TOKIO_THREADS;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::runtime::Builder;

    // TODO: evaluate std::thread::available_concurrency as a num_cpus replacement
    let cores = 8;
    let tokio_runtime = Arc::new(
        Builder::new_multi_thread()
            .enable_all()
            .worker_threads((cores / 4).max(MIN_RECOMMENDED_TOKIO_THREADS))
            .thread_name_fn(|| {
                static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
                let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
                format!("tokio-voxygen-{}", id)
            })
            .build()
            .unwrap(),
    );

    // Setup audio
    let mut audio = match settings.audio.output {
        AudioOutput::Off => AudioFrontend::no_audio(),
        AudioOutput::Automatic => AudioFrontend::new(settings.audio.num_sfx_channels),
        //    AudioOutput::Device(ref dev) => Some(dev.clone()),
    };

    audio.set_master_volume(settings.audio.master_volume);
    audio.set_music_volume(settings.audio.music_volume);
    audio.set_sfx_volume(settings.audio.sfx_volume);

    // Load the profile.
    let profile = Profile::load();

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

    //创建运行窗体
    let (mut window, event_loop) = match Window::new(&settings, &tokio_runtime) {
        Ok(ok) => ok,
        Err(error) => panic!("Failed to create window!: {:?}", error),
    };

    let clipboard = iced::Clipboard::connect(window.window());

    let lazy_init = SpriteRenderContext::new(window.renderer_mut());

    let global_state = GlobalState {
        audio,
        profile,
        window,
        tokio_runtime,
        lazy_init,
        clock: Clock::new(std::time::Duration::from_secs_f64(
            1.0 / get_fps(settings.graphics.max_fps) as f64,
        )),
        settings,
        info_message: None,
        i18n,
        clipboard,
        client_error: None,
        clear_shadows_next_frame: false,
    };

    run::run(global_state, event_loop);
}


fn init_log () {

    #[cfg(feature = "wasm")]
    {
        wasm_logger::init(wasm_logger::Config::default());
    }
    
    #[cfg(not(feature = "wasm"))]
    {
        let mut builder = env_logger::Builder::new();
        builder.filter_module("wgpu", log::LevelFilter::Warn);
        builder.filter_module("wgpu_core", log::LevelFilter::Warn);
        builder.filter_level(log::LevelFilter::Info);
        builder.init();
    }

    log::info!("inited log");
}