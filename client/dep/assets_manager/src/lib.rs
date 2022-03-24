
extern crate self as assets_manager;

pub mod asset;
pub use asset::{Asset, Compound};

mod cache;
pub use cache::AssetCache;

mod dirs;
pub use dirs::DirHandle;

mod error;
pub use error::{BoxedError, Error};

pub mod loader;

mod entry;
pub use entry::{AssetGuard, Handle};

mod key;

pub mod source;

mod utils;
pub use utils::{SharedBytes, SharedString};
