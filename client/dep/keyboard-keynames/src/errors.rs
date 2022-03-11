//! Errors during creation of a KeyLayout
use std::{error::Error, fmt::{self, Display}};


/// Errors during creation of a KeyLayout
#[derive(Debug)]
pub enum KeyLayoutError {
    /// An error with Wayland
    WaylandError,
    /// An error with X11
    X11Error,
    /// An error determining the session type
    SessionError,
    /// The platform is unsupported
    PlatformUnsupportedError,

}

impl Error for KeyLayoutError {}
impl Display for KeyLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            KeyLayoutError::WaylandError => {
                write!(f, "Error getting KeyLayout from Wayland compositor")
            }
            KeyLayoutError::X11Error => write!(f, "Error getting KeyLayout from X11 server"),
            KeyLayoutError::SessionError => write!(f, "Error getting XDG_SESSION_TYPE"),
            KeyLayoutError::PlatformUnsupportedError => write!(f, "Your platform is not supported"),
        }
    }
}