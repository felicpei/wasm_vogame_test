use crate::{
    menu::main::MainMenuState,
    settings::get_fps,
    ui,
    window::{Event, EventLoop},
    Direction, GlobalState, PlayState, PlayStateResult,
};

use std::mem;
use instant::Duration;

pub fn run(mut global_state: GlobalState, event_loop: EventLoop) {
    
    log::info!("start game run");
    // Set up the initial play state.
    let mut states: Vec<Box<dyn PlayState>> = vec![Box::new(MainMenuState::new(&mut global_state))];
    states.last_mut().map(|current_state| {
        current_state.enter(&mut global_state, Direction::Forwards);
        let current_state = current_state.name();
        log::info!("{:?} Started game with state", current_state);
    });

    log::info!("start game new menu over");
    // Used to ignore every other `MainEventsCleared`
    // This is a workaround for a bug on macos in which mouse motion events are only
    // reported every other cycle of the event loop
    // See: https://github.com/rust-windowing/winit/issues/1418
    let mut polled_twice = false;

    event_loop.run(move |event, _, control_flow| {
        // Continuously run loop since we handle sleeping
        *control_flow = winit::event_loop::ControlFlow::Poll;

        // Don't pass resize events to the ui, `Window` is responsible for:
        // - deduplicating them
        // - generating resize events for the ui
        // - ensuring consistent sizes are passed to the ui and to the renderer
        if !matches!(&event, winit::event::Event::WindowEvent {
            event: winit::event::WindowEvent::Resized(_),
            ..
        }) {
            // Get events for the ui.
            if let Some(event) = ui::Event::try_from(&event, global_state.window.window()) {
                global_state.window.send_event(Event::Ui(event));
            }
            // iced ui events
            // TODO: no clone
            if let winit::event::Event::WindowEvent { event, .. } = &event {
                let window = &mut global_state.window;
                if let Some(event) = ui::ice::window_event(event, window.scale_factor(), window.modifiers())
                {
                    window.send_event(Event::IcedUi(event));
                }
            }
        }

        match event {
            winit::event::Event::NewEvents(_) => {
               
            },
            winit::event::Event::MainEventsCleared => {
                if polled_twice {
                    handle_main_events_cleared(&mut states, control_flow, &mut global_state);
                }
                polled_twice = !polled_twice;
            },
            winit::event::Event::WindowEvent { event, .. } => {
                
                if let winit::event::WindowEvent::Focused(focused) = event {
                    global_state.audio.set_master_volume(if focused {
                        global_state.settings.audio.master_volume
                    } else {
                        global_state.settings.audio.inactive_master_volume_perc
                            * global_state.settings.audio.master_volume
                    });
                }

                global_state
                    .window
                    .handle_window_event(event, &mut global_state.settings)
            },
            winit::event::Event::DeviceEvent { event, .. } => {
                global_state.window.handle_device_event(event)
            },
            winit::event::Event::LoopDestroyed => {
               
                //save
                global_state.settings.save();
                global_state.profile.save();
            },
            _ => {},
        }
    });
}

fn handle_main_events_cleared(
    states: &mut Vec<Box<dyn PlayState>>,
    control_flow: &mut winit::event_loop::ControlFlow,
    global_state: &mut GlobalState,
) {
    // Screenshot / Fullscreen toggle
    global_state
        .window
        .resolve_deduplicated_events(&mut global_state.settings);
    
    let mut exit = true;
    while let Some(state_result) = states.last_mut().map(|last| {
        let events = global_state.window.fetch_events();
        last.tick(global_state, events)
    }) {
        // Implement state transfer logic.
        match state_result {
            PlayStateResult::Continue => {
                exit = false;
                break;
            },
            PlayStateResult::Shutdown => {
                log::debug!("Shutting down all states...");
                while states.last().is_some() {
                    states.pop().map(|old_state| {
                        log::debug!("Popped state '{}'.", old_state.name());
                        global_state.on_play_state_changed();
                    });
                }
            },
            PlayStateResult::Pop => {
                states.pop().map(|old_state| {
                    log::info!("Popped state '{}'.", old_state.name());
                    global_state.on_play_state_changed();
                });
                states.last_mut().map(|new_state| {
                    new_state.enter(global_state, Direction::Backwards);
                });
            },
            PlayStateResult::Push(mut new_state) => {
                new_state.enter(global_state, Direction::Forwards);
                log::info!("Pushed state '{}'.", new_state.name());
                states.push(new_state);
                global_state.on_play_state_changed();
            },
            PlayStateResult::Switch(mut new_state) => {
                new_state.enter(global_state, Direction::Forwards);
                states.last_mut().map(|old_state| {
                    log::info!(
                        "Switching to state '{}' from state '{}'.",
                        new_state.name(),
                        old_state.name()
                    );
                    mem::swap(old_state, &mut new_state);
                    global_state.on_play_state_changed();
                });
            },
        }
    }

    if exit {
        *control_flow = winit::event_loop::ControlFlow::Exit;
    }

    let mut capped_fps = false;

    if let Some(last) = states.last_mut() {
        capped_fps = last.capped_fps();

        let renderer_mut = global_state.window.renderer_mut();

        match renderer_mut.surface.get_current_texture() {

            Ok(frame) => {
                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = renderer_mut.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("A render encoder"),
                });
    
                //per frame
                if let Some(mut drawer) = renderer_mut.start_recording_frame(last.globals_bind_group(), &mut encoder, &view)
                                            .expect("Unrecoverable render error when starting a new frame!") {
                    
                    if global_state.clear_shadows_next_frame {
                        drawer.clear_shadows();
                    }

                    last.render(&mut drawer, &global_state.settings);

                    renderer_mut.queue.submit(Some(encoder.finish()));
                    frame.present();

                };

                if global_state.clear_shadows_next_frame {
                    global_state.clear_shadows_next_frame = false;
                }
            },
            Err(err @ wgpu::SurfaceError::Lost) => {
                log::warn!("{}. SurfaceError::Lost swap chain. A frame will be missed", err);
                renderer_mut.on_resize(renderer_mut.resolution);
            },
            Err(wgpu::SurfaceError::Timeout) => {
                // This will probably be resolved on the next frame
                // NOTE: we don't log this because it happens very frequently with
                // PresentMode::Fifo and unlimited FPS on certain machines
            },
            Err(err @ wgpu::SurfaceError::Outdated) => {
                log::warn!("{}. SurfaceError::Outdated the swapchain", err);
                //self.swap_chain = self.device.create_swap_chain(&self.surface, &self.sc_desc);
                renderer_mut.surface.configure(&renderer_mut.device, &renderer_mut.sc_desc);
            },
            Err(_err @ wgpu::SurfaceError::OutOfMemory) => {
                panic!("Swapchain error: OutOfMemory. Rendering cannot continue.");
            }
        };
    }

    if !exit {

        // Enforce an FPS cap for the non-game session play states to prevent them
        // running at hundreds/thousands of FPS resulting in high GPU usage for
        // effectively doing nothing.
        let max_fps = get_fps(global_state.settings.graphics.max_fps);
        let max_background_fps = get_fps(global_state.settings.graphics.max_background_fps);
        const TITLE_SCREEN_FPS_CAP: u32 = 60;
        let target_fps = if !global_state.window.focused {
            u32::min(max_background_fps, max_fps)
        } else if capped_fps {
            u32::min(TITLE_SCREEN_FPS_CAP, max_fps)
        } else {
            max_fps
        };

        global_state.clock.set_target_dt(Duration::from_secs_f64(1.0 / target_fps as f64));
        global_state.clock.tick();

        // Maintain global state.
        global_state.maintain(global_state.clock.dt());
    }
}
