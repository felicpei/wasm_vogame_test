use crate::errors::KeyLayoutError;

/// The KeyLayout struct (does not work on Mac, only Linux and Windows currently)
pub struct KeyLayout;


impl KeyLayout {
    
    /// Construct a KeyLayout from a winit Window
    pub fn new_from_window(_window: &winit::window::Window) -> Result<KeyLayout, KeyLayoutError> {
        Err(KeyLayoutError::PlatformUnsupportedError)
    }

    /// Construct a KeyLayout
    pub fn new() -> Result<KeyLayout, KeyLayoutError> {
        Err(KeyLayoutError::PlatformUnsupportedError)
    }

    /// Convert a scancode to a String
    pub fn get_key_as_string(&self, scancode: u32) -> String {
        panic!("Unimplemented")
    }
}