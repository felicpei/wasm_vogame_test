mod client_init;
mod scene;
mod ui;

use super::char_selection::CharSelectionState;
use crate::{
    render::{Drawer, GlobalsBindGroup},
    settings::Settings,
    window::Event,
    Direction, GlobalState, PlayState, PlayStateResult,
};
use client::{
    addr::ConnectionArgs,
    error::{InitProtocolError, NetworkConnectError, NetworkError},
    Client, ServerInfo,
};
use client_init::{ClientInit, Error as InitError, Msg as InitMsg};
use common::comp;
use i18n::LocalizationHandle;
use scene::Scene;
use std::sync::Arc;
use tokio::runtime;

use ui::{Event as MainMenuEvent, MainMenuUi};

// TODO: show status messages for waiting on server creation, client init, and
// pipeline creation (we can show progress of pipeline creation)
enum InitState {
    None,
    // Waiting on the client initialization
    Client(ClientInit),
    // Client initialized but still waiting on Renderer pipeline creation
    Pipeline(Box<Client>),
}

impl InitState {
    fn client(&self) -> Option<&ClientInit> {
        if let Self::Client(client_init) = &self {
            Some(client_init)
        } else {
            None
        }
    }
}

pub struct MainMenuState {
    main_menu_ui: MainMenuUi,
    init: InitState,
    scene: Scene,
}

impl MainMenuState {
    /// Create a new `MainMenuState`.
    pub fn new(global_state: &mut GlobalState) -> Self {
        Self {
            main_menu_ui: MainMenuUi::new(global_state),
            init: InitState::None,
            scene: Scene::new(global_state.window.renderer_mut()),
        }
    }
}

impl PlayState for MainMenuState {
    fn enter(&mut self, global_state: &mut GlobalState, _: Direction) {

        log::info!("PlayState for MainMenuState : enter");

        // Kick off title music
        if global_state.settings.audio.output.is_enabled() && global_state.audio.music_enabled() {
            global_state.audio.play_title_music();
        }

        // Updated localization in case the selected language was changed
        self.main_menu_ui
            .update_language(global_state.i18n, &global_state.settings);
        // Set scale mode in case it was change
        self.main_menu_ui
            .set_scale_mode(global_state.settings.interface.ui_scale);
    }

    #[allow(clippy::single_match)] // TODO: remove when event match has multiple arms
    fn tick(&mut self, global_state: &mut GlobalState, events: Vec<Event>) -> PlayStateResult {
        
        //log::info!("PlayState for MainMenuState : tic");
        // Pull in localizations
        let localized_strings = &global_state.i18n.read();
       
        // Handle window events.
        for event in events {
            // Pass all events to the ui first.
            if self.main_menu_ui.handle_event(event.clone()) {
                continue;
            }

            match event {
                Event::Close => return PlayStateResult::Shutdown,
                // Ignore all other events.
                _ => {},
            }
        }
        // Poll client creation.
        match self.init.client().and_then(|init| init.poll()) {
            Some(InitMsg::Done(Ok(mut client))) => {
                // Register voxygen components / resources
                crate::ecs::init(client.state_mut().ecs_mut());
                self.init = InitState::Pipeline(Box::new(client));
            },
            Some(InitMsg::Done(Err(e))) => {
                self.init = InitState::None;
                log::error!("{:?} Client Init failed raw error", e);
                let e = get_client_msg_error(e, &global_state.i18n);
                // Log error for possible additional use later or in case that the error
                // displayed is cut of.
                log::error!("{:?}  Client Init failed", e);
                global_state.info_message = Some(
                    localized_strings
                        .get("main.login.client_init_failed")
                        .to_owned()
                        .replace("{init_fail_reason}", e.as_str()),
                );
            },
           
            None => {},
        }

        // Tick the client to keep the connection alive if we are waiting on pipelines
        if let InitState::Pipeline(client) = &mut self.init {
            match client.tick(
                comp::ControllerInputs::default(),
                global_state.clock.dt(),
                |_| {},
            ) {
                Ok(events) => {
                    for event in events {
                        match event {
                            client::Event::SetViewDistance(vd) => {
                                global_state.settings.graphics.view_distance = vd;
                                global_state.settings.save();
                            },
                            client::Event::Disconnect => {
                                global_state.info_message = Some(
                                    localized_strings
                                        .get("main.login.server_shut_down")
                                        .to_owned(),
                                );
                                self.init = InitState::None;
                            },
                            _ => {},
                        }
                    }
                },
                Err(err) => {
                    global_state.info_message =
                        Some(localized_strings.get("common.connection_lost").to_owned());
                    log::error!("{:?}    [main menu] Failed to tick the client", err);
                    self.init = InitState::None;
                },
            }
        }

        // Poll renderer pipeline creation
        if let InitState::Pipeline(..) = &self.init {
            // If not complete go to char select screen
            // Always succeeds since we check above
            if let InitState::Pipeline(client) =
                core::mem::replace(&mut self.init, InitState::None)
            {
                self.main_menu_ui.connected();
                return PlayStateResult::Push(Box::new(CharSelectionState::new(
                    global_state,
                    std::rc::Rc::new(std::cell::RefCell::new(*client)),
                )));
            }
        }

        // Maintain the UI.
        for event in self
            .main_menu_ui
            .maintain(global_state, global_state.clock.dt())
        {
            match event {

                MainMenuEvent::LoginAttempt {
                    username,
                    password,
                    server_address,
                } => {
                    let mut net_settings = &mut global_state.settings.networking;
                    net_settings.username = username.clone();
                    net_settings.default_server = server_address.clone();
                    if !net_settings.servers.contains(&server_address) {
                        net_settings.servers.push(server_address.clone());
                    }

                    global_state.settings.save();

                    //初始化网络
                    let connection_args = ConnectionArgs::Tcp {
                        hostname: server_address,
                    };

                    log::info!("### try MainMenuEvent => LoginAttempt");
                    attempt_login(
                        &mut global_state.info_message,
                        username,
                        password,
                        connection_args,
                        &mut self.init,
                        &global_state.tokio_runtime,
                        &global_state.i18n,
                    );
                },

                MainMenuEvent::CancelLoginAttempt => {
                    self.init = InitState::None;
                    self.main_menu_ui.cancel_connection();
                },
                MainMenuEvent::ChangeLanguage(new_language) => {
                    global_state.settings.language.selected_language =
                        new_language.language_identifier;
                    global_state.i18n = LocalizationHandle::load_expect(
                        &global_state.settings.language.selected_language,
                    );
                    global_state.i18n.read().log_missing_entries();
                    global_state
                        .i18n
                        .set_english_fallback(global_state.settings.language.use_english_fallback);
                    self.main_menu_ui
                        .update_language(global_state.i18n, &global_state.settings);
                },
                
                MainMenuEvent::Quit => return PlayStateResult::Shutdown,
                // Note: Keeping in case we re-add the disclaimer
                /*MainMenuEvent::DisclaimerAccepted => {
                    global_state.settings.show_disclaimer = false
                },*/
               
                MainMenuEvent::DeleteServer { server_index } => {
                    let net_settings = &mut global_state.settings.networking;
                    net_settings.servers.remove(server_index);

                    global_state.settings.save();
                },
            }
        }

        if let Some(info) = global_state.info_message.take() {
            self.main_menu_ui.show_info(info);
        }

        PlayStateResult::Continue
    }

    fn name(&self) -> &'static str { "Title" }

    fn capped_fps(&self) -> bool { true }

    fn globals_bind_group(&self) -> &GlobalsBindGroup { self.scene.global_bind_group() }

    fn render<'a>(&'a self, drawer: &mut Drawer<'a>, _: &Settings) {

        //log::info!("PlayState for MainMenuState : render");

        // Draw the UI to the screen.
        let mut third_pass = drawer.third_pass();
        third_pass.init_ui();
        self.main_menu_ui.render(&mut third_pass);
    }
}

fn get_client_msg_error(
    error: client_init::Error,
    localized_strings: &LocalizationHandle,
) -> String {
    let localization = localized_strings.read();

    // When a network error is received and there is a mismatch between the client
    // and server version it is almost definitely due to this mismatch rather than
    // a true networking error.
    let net_error = |error: String, mismatched_server_info: Option<ServerInfo>| -> String {
        if let Some(server_info) = mismatched_server_info {
            format!(
                "{} {}: {} ({}) {}: {} ({})",
                localization.get("main.login.network_wrong_version"),
                localization.get("main.login.client_version"),
                &*common::util::GIT_HASH,
                &*common::util::GIT_DATE,
                localization.get("main.login.server_version"),
                server_info.git_hash,
                server_info.git_date,
            )
        } else {
            format!(
                "{}: {}",
                localization.get("main.login.network_error"),
                error
            )
        }
    };

    use client::Error;
    match error {
        InitError::ClientError {
            error,
            mismatched_server_info,
        } => match error {
            Error::SpecsErr(e) => {
                format!("{}: {}", localization.get("main.login.internal_error"), e)
            },
            Error::AuthErr(e) => format!(
                "{}: {}",
                localization.get("main.login.authentication_error"),
                e
            ),
            Error::Kicked(e) => e,
            Error::TooManyPlayers => localization.get("main.login.server_full").into(),
            Error::AuthServerNotTrusted => {
                localization.get("main.login.untrusted_auth_server").into()
            },
            Error::ServerTimeout => localization.get("main.login.timeout").into(),
            Error::ServerShutdown => localization.get("main.login.server_shut_down").into(),
            Error::NotOnWhitelist => localization.get("main.login.not_on_whitelist").into(),
            Error::Banned(reason) => {
                format!("{}: {}", localization.get("main.login.banned"), reason)
            },
            Error::InvalidCharacter => localization.get("main.login.invalid_character").into(),
            Error::NetworkErr(NetworkError::ConnectFailed(NetworkConnectError::Handshake(
                InitProtocolError::WrongVersion(_),
            ))) => net_error(
                localization
                    .get("main.login.network_wrong_version")
                    .to_owned(),
                mismatched_server_info,
            ),
            Error::NetworkErr(e) => net_error(e.to_string(), mismatched_server_info),
            Error::ParticipantErr(e) => net_error(e.to_string(), mismatched_server_info),
            Error::StreamErr(e) => net_error(e.to_string(), mismatched_server_info),
            Error::HostnameLookupFailed(e) => {
                format!("{}: {}", localization.get("main.login.server_not_found"), e)
            },
            Error::Other(e) => {
                format!("{}: {}", localization.get("common.error"), e)
            },
            Error::AuthServerUrlInvalid(e) => {
                format!(
                    "{}: https://{}",
                    localization.get("main.login.failed_auth_server_url_invalid"),
                    e
                )
            },
        },
        InitError::ClientCrashed => localization.get("main.login.client_crashed").into(),
        InitError::ServerNotFound => localization.get("main.login.server_not_found").into(),
    }
}

//登录
fn attempt_login(
    info_message: &mut Option<String>,
    username: String,
    password: String,
    connection_args: ConnectionArgs,
    init: &mut InitState,
    runtime: &Arc<runtime::Runtime>,
    localized_strings: &LocalizationHandle,
) {
    log::info!("##### attempt_login start");
    let localization = localized_strings.read();
    if let Err(err) = comp::Player::alias_validate(&username) {
        match err {
            comp::AliasError::ForbiddenCharacters => {
                *info_message = Some(
                    localization
                        .get("main.login.username_bad_characters")
                        .to_owned(),
                );
            },
            comp::AliasError::TooLong => {
                *info_message = Some(
                    localization
                        .get("main.login.username_too_long")
                        .to_owned()
                        .replace("{max_len}", comp::MAX_ALIAS_LEN.to_string().as_str()),
                );
            },
        }
        return;
    }

    // Don't try to connect if there is already a connection in progress.
    if let InitState::None = init {
        *init = InitState::Client(ClientInit::new(
            connection_args,
            username,
            password,
            Arc::clone(runtime),
        ));
    }
}
