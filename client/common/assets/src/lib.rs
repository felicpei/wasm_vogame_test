use dot_vox::DotVoxData;
use image::DynamicImage;
use lazy_static::lazy_static;

use std::{
    borrow::Cow,
    sync::Arc,
    collections::HashMap,
    sync::Mutex,
    fmt,
};

pub use assets_manager::{
    asset::{DirLoadable, Ron},
    loader::{
        self, BincodeLoader, BytesLoader, JsonLoader, LoadFrom, Loader, RonLoader, StringLoader,
    },
    source::{self, Source},
    Asset, AssetCache, BoxedError, Compound, Error, SharedString,
};
mod fs;


lazy_static! {
    
    static ref ASSETS: AssetCache<fs::ResSystem> =  AssetCache::with_source(fs::ResSystem::new().unwrap());

    static ref ASSET_MAP: Mutex<HashMap<String, Vec<u8>>> = Mutex::new({ HashMap::new() });

    static ref ASSET_MAP_DIR: Mutex<HashMap<String, bool>> = Mutex::new({ HashMap::new() });
}

pub enum ResourceError {
    GetMapError,
    NotExists(String),
}

impl fmt::Debug for ResourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GetMapError => {
                f.debug_tuple("Get Resources => GetMapError").finish()
            },

            Self::NotExists(err) => {
                f.debug_tuple("Get Resources => File Not Exists").field(err).finish()
            },
        }
    }
}

//缓存dir数据
pub fn set_cache_dir(name: &str) {
    ASSET_MAP_DIR.lock().unwrap().insert(name.to_string(), true);
}

//缓存data, 通过js传入
pub fn set_cache_data(name: &str, data: &[u8]) {
    let vec = data.to_vec();
    let name_str = name.to_string();
    ASSET_MAP.lock().unwrap().insert(name_str, vec);
}

//获取缓存data
pub fn get_cache_data<'a,'b>(id: &'a str, ext: &'a str) -> Result<Cow<'b, [u8]>,ResourceError>  {
    let mut name = String::from(id);
    name.push_str(&".");
    name.push_str(ext);

    let map = match ASSET_MAP.lock() {
        Ok(map) => map,
        Err(err) =>{
            log::error!("get_cache_data error, get map error: {:?}", err);
            return Err(ResourceError::GetMapError);
        }
    };

    let bytes = match map.get(&name) {
        Some(bytes) =>{
            bytes
        },
        None =>{
            return Err(ResourceError::NotExists(name));
        }
    };

    let len = bytes.len();
    let mut ret = vec![0; len];
    for index in 0..len {
        ret[index] = bytes[index];
    }
    Ok(Cow::Owned(ret))
}


pub type AssetHandle<T> = assets_manager::Handle<'static, T>;
pub type AssetGuard<T> = assets_manager::AssetGuard<'static, T>;
pub type AssetDirHandle<T> = assets_manager::DirHandle<'static, T, fs::ResSystem>;


/// The Asset trait, which is implemented by all structures that have their data
/// stored in the filesystem.
pub trait AssetExt: Sized + Send + Sync + 'static {
    /// Function used to load assets from the filesystem or the cache.
    /// Example usage:
    /// ```no_run
    /// use veloren_common_assets::{AssetExt, Image};
    ///
    /// let my_image = Image::load("core.ui.backgrounds.city").unwrap();
    /// ```
    fn load(specifier: &str) -> Result<AssetHandle<Self>, Error>;

    /// Function used to load assets from the filesystem or the cache and return
    /// a clone.
    fn load_cloned(specifier: &str) -> Result<Self, Error>
    where
        Self: Clone,
    {
        Self::load(specifier).map(AssetHandle::cloned)
    }

    fn load_or_insert_with(
        specifier: &str,
        default: impl FnOnce(Error) -> Self,
    ) -> AssetHandle<Self> {
        Self::load(specifier).unwrap_or_else(|err| Self::get_or_insert(specifier, default(err)))
    }

    /// Function used to load essential assets from the filesystem or the cache.
    /// It will panic if the asset is not found. Example usage:
    /// ```no_run
    /// use veloren_common_assets::{AssetExt, Image};
    ///
    /// let my_image = Image::load_expect("core.ui.backgrounds.city");
    /// ```
    #[track_caller]
    fn load_expect(specifier: &str) -> AssetHandle<Self> {
        #[track_caller]
        #[cold]
        fn expect_failed(err: Error) -> ! {
            panic!(
                "Failed loading essential asset: {} (error={:?})",
                err.id(),
                err.reason()
            )
        }

        // Avoid using `unwrap_or_else` to avoid breaking `#[track_caller]`
        match Self::load(specifier) {
            Ok(handle) => handle,
            Err(err) => expect_failed(err),
        }
    }

    /// Function used to load essential assets from the filesystem or the cache
    /// and return a clone. It will panic if the asset is not found.
    #[track_caller]
    fn load_expect_cloned(specifier: &str) -> Self
    where
        Self: Clone,
    {
        Self::load_expect(specifier).cloned()
    }

    fn load_owned(specifier: &str) -> Result<Self, Error>;

    fn get_or_insert(specifier: &str, default: Self) -> AssetHandle<Self>;
}

/// Loads directory and all files in it
///
/// # Errors
/// An error is returned if the given id does not match a valid readable
/// directory.
///
/// When loading a directory recursively, directories that can't be read are
/// ignored.
pub fn load_dir<T: DirLoadable>(
    specifier: &str,
    recursive: bool,
) -> Result<AssetDirHandle<T>, Error> {
    let specifier = specifier.strip_suffix(".*").unwrap_or(specifier);
    ASSETS.load_dir(specifier, recursive)
}


impl<T: Compound> AssetExt for T {
    fn load(specifier: &str) -> Result<AssetHandle<Self>, Error> { ASSETS.load(specifier) }

    fn load_owned(specifier: &str) -> Result<Self, Error> { ASSETS.load_owned(specifier) }

    fn get_or_insert(specifier: &str, default: Self) -> AssetHandle<Self> {
        ASSETS.get_or_insert(specifier, default)
    }
}

pub struct Image(pub Arc<DynamicImage>);

impl Image {
    pub fn to_image(&self) -> Arc<DynamicImage> { Arc::clone(&self.0) }
}

pub struct ImageLoader;
impl Loader<Image> for ImageLoader {
    fn load(content: Cow<[u8]>, ext: &str) -> Result<Image, BoxedError> {
        let format = image::ImageFormat::from_extension(ext)
            .ok_or_else(|| format!("Invalid file extension {}", ext))?;
        let image = image::load_from_memory_with_format(&content, format)?;
        Ok(Image(Arc::new(image)))
    }
}

impl Asset for Image {
    type Loader = ImageLoader;
    const EXTENSIONS: &'static [&'static str] = &["png"];
}

pub struct DotVoxAsset(pub DotVoxData);

pub struct DotVoxLoader;
impl Loader<DotVoxAsset> for DotVoxLoader {
    fn load(content: std::borrow::Cow<[u8]>, _: &str) -> Result<DotVoxAsset, BoxedError> {
        let data = dot_vox::load_bytes(&content).map_err(|err| err.to_owned())?;
        Ok(DotVoxAsset(data))
    }
}

impl Asset for DotVoxAsset {
    type Loader = DotVoxLoader;
    const EXTENSION: &'static str = "vox";
}
