use serde::{Deserialize, Serialize};

pub mod audio;
pub mod chat;
pub mod control;
pub mod gamepad;
pub mod gameplay;
pub mod graphics;
pub mod interface;
pub mod language;
pub mod networking;

pub use audio::{AudioOutput, AudioSettings};
pub use chat::ChatSettings;
pub use control::ControlSettings;
pub use gamepad::GamepadSettings;
pub use gameplay::GameplaySettings;
pub use graphics::{get_fps, Fps, GraphicsSettings};
pub use interface::InterfaceSettings;
pub use language::LanguageSettings;
pub use networking::NetworkingSettings;

/// `Settings` contains everything that can be configured in the settings.ron
/// file.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub chat: ChatSettings,
    pub controls: ControlSettings,
    pub interface: InterfaceSettings,
    pub gameplay: GameplaySettings,
    pub networking: NetworkingSettings,
    pub graphics: GraphicsSettings,
    pub audio: AudioSettings,
    pub show_disclaimer: bool,
    pub send_logon_commands: bool,
    // TODO: Remove at a later date, for dev testing
    pub logon_commands: Vec<String>,
    pub language: LanguageSettings,
    pub controller: GamepadSettings,
}

impl Default for Settings {
    fn default() -> Self {
        let settings = Settings {
            chat: ChatSettings::default(),
            controls: ControlSettings::default(),
            interface: InterfaceSettings::default(),
            gameplay: GameplaySettings::default(),
            networking: NetworkingSettings::default(),
            graphics: GraphicsSettings::default(),
            audio: AudioSettings::default(),
            show_disclaimer: true,
            send_logon_commands: false,
            logon_commands: Vec::new(),
            language: LanguageSettings::default(),
            controller: GamepadSettings::default(),
        };
        settings.save();
        settings
    }
}

impl Settings {
    pub fn load() -> Self {
     
        log::warn!("todo load setting data, 目前仅使用默认");

        //wasm 仅用默认
        let default_settings = Self::default();
        default_settings
    }

    pub fn save(&self) {
        log::warn!("todo save setting data");
    }

    pub fn display_warnings(&self) {
        if !self.graphics.render_mode.experimental_shaders.is_empty() {
            log::warn!(
                "One or more experimental shaders are enabled, all rendering guarantees are off. \
                 Experimental shaders may be unmaintained, mutually-incompatible, entirely \
                 broken, or may cause your GPU to explode. You have been warned!"
            );
        }
    }
}
