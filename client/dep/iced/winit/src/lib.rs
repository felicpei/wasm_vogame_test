#![forbid(unsafe_code)]
#![forbid(rust_2018_idioms)]

#[doc(no_inline)]

pub use iced_native as native;
pub use iced_native::*;
pub use winit;
pub use iced_graphics as graphics;

pub mod application;
pub mod conversion;
pub mod settings;

pub mod clipboard;
mod error;
mod mode;
mod proxy;

pub use application::Application;
pub use clipboard::Clipboard;
pub use error::Error;
pub use mode::Mode;
pub use proxy::Proxy;
pub use settings::Settings;

pub use iced_graphics::Viewport;
