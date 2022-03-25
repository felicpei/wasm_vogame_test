
#[cfg(feature = "ab_glyph")]
mod fonts;

pub use crate::dirs::DirLoadable;

#[allow(unused)]
use crate::{
    cache::load_from_source,
    entry::CacheEntry,
    loader,
    source::Source,
    utils::{PrivateMarker, SharedBytes, SharedString},
    AssetCache, BoxedError, Error,
};

#[cfg(feature = "serde")]
#[allow(unused)]
use serde::{Deserialize, Serialize};

#[allow(unused)]
use std::{borrow::Cow, io, sync::Arc};

pub trait Asset: Sized + Send + Sync + 'static {

    const EXTENSION: &'static str = "";

    const EXTENSIONS: &'static [&'static str] = &[Self::EXTENSION];

    type Loader: loader::Loader<Self>;

    
    #[inline]
    #[allow(unused_variables)]
    fn default_value(id: &str, error: Error) -> Result<Self, Error> {
        Err(error)
    }
}

impl<A> Asset for Box<A>
where
    A: Asset,
{
    const EXTENSIONS: &'static [&'static str] = A::EXTENSIONS;
    type Loader = loader::LoadFromAsset<A>;

    #[inline]
    fn default_value(id: &str, error: Error) -> Result<Box<A>, Error> {
        A::default_value(id, error).map(Box::new)
    }
}

impl<A> NotHotReloaded for Box<A> where A: Asset + NotHotReloaded {}

pub trait Compound: Sized + Send + Sync + 'static {
    fn load<S: Source + ?Sized>(cache: &AssetCache<S>, id: &str) -> Result<Self, BoxedError>;

    fn _load_and_record<S: Source + ?Sized, P: PrivateMarker>(
        cache: &AssetCache<S>,
        id: &SharedString,
    ) -> Result<Self, Error> {
        let res = Self::load(cache, id);
        res.map_err(|err| Error::new(id.clone(), err))
    }

    fn _load_and_record_entry<S: Source + ?Sized, P: PrivateMarker>(
        cache: &AssetCache<S>,
        id: SharedString,
    ) -> Result<CacheEntry, Error> {
        let asset = Self::_load_and_record::<S, P>(cache, &id)?;
        Ok(CacheEntry::new(asset, id))
    }
    
    fn get_key<P: PrivateMarker>() -> Option<crate::key::AssetType> {
        None
    }
}

impl<A> Compound for A
where
    A: Asset,
{
    #[inline]
    fn load<S: Source + ?Sized>(cache: &AssetCache<S>, id: &str) -> Result<Self, BoxedError> {
        Ok(load_from_source(cache.source(), id)?)
    }

    
    fn _load_and_record<S: Source + ?Sized, P: PrivateMarker>(
        cache: &AssetCache<S>,
        id: &SharedString,
    ) -> Result<Self, Error> {
        let asset = load_from_source(cache.source(), id)?;
        Ok(asset)
    }

    
    fn get_key<P: PrivateMarker>() -> Option<crate::key::AssetType> {
        Some(crate::key::AssetType::of::<Self>())
    }
}

impl<A> Compound for Arc<A>
where
    A: Compound,
{
    fn load<S: Source + ?Sized>(cache: &AssetCache<S>, id: &str) -> Result<Self, BoxedError> {
        let asset = cache.load_owned::<A>(id)?;
        Ok(Arc::new(asset))
    }
}

impl<A> NotHotReloaded for Arc<A> where A: Compound + NotHotReloaded {}

pub trait NotHotReloaded: Storable {}

pub trait Storable: Send + Sync + 'static {

    fn get_key<P: PrivateMarker>() -> Option<crate::key::AssetType> {
        None
    }
}

impl<A> Storable for A
where
    A: Compound,
{

    fn get_key<P: PrivateMarker>() -> Option<crate::key::AssetType> {
        Self::get_key::<P>()
    }
}

macro_rules! string_assets {
    ( $( $typ:ty, )* ) => {
        $(
            impl Asset for $typ {
                const EXTENSION: &'static str = "txt";
                type Loader = loader::StringLoader;
            }
        )*
    }
}

string_assets! {
    String, Box<str>, SharedString,
}

macro_rules! impl_storable {
    ( $( $typ:ty, )* ) => {
        $(
            impl Storable for $typ {}
            impl NotHotReloaded for $typ {}
        )*
    }
}

impl_storable! {
    i8, i16, i32, i64, i128, isize,
    u8, u16, u32, u64, u128, usize,
    f32, f64, char, &'static str,
    SharedBytes,
}

impl<A: Send + Sync + 'static> Storable for Vec<A> {}
impl<A: Send + Sync + 'static> NotHotReloaded for Vec<A> {}
impl<A: Send + Sync + 'static> Storable for &'static [A] {}
impl<A: Send + Sync + 'static> NotHotReloaded for &'static [A] {}

macro_rules! serde_assets {
    (
        $(
            #[cfg(feature = $feature:literal)]
            struct $name:ident => (
                $loader:path,
                [$($ext:literal),*],
            );
        )*
    ) => {
        $(
            ///
            /// This type can directly be used as an [`Asset`] to load values
            /// from an [`AssetCache`]. This is useful to load assets external
            /// types without a newtype wrapper (eg [`Vec`]).
            #[cfg(feature = $feature)]
            #[cfg_attr(docsrs, doc(cfg(feature = $feature)))]
            #[derive(Debug, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
            #[serde(transparent)]
            #[repr(transparent)]
            pub struct $name<T>(pub T);

            #[cfg(feature = $feature)]
            impl<T> Clone for $name<T>
            where
                T: Clone
            {
                fn clone(&self) -> Self {
                    Self(self.0.clone())
                }

                fn clone_from(&mut self, other: &Self) {
                    self.0.clone_from(&other.0)
                }
            }

            #[cfg(feature = $feature)]
            impl<T> From<T> for $name<T> {
                #[inline]
                fn from(t: T) -> Self {
                    Self(t)
                }
            }

            #[cfg(feature = $feature)]
            impl<T> $name<T> {
                /// Unwraps the inner value.
                #[inline]
                pub fn into_inner(self) -> T {
                    self.0
                }
            }

            #[cfg(feature = $feature)]
            #[cfg_attr(docsrs, doc(cfg(feature = $feature)))]
            impl<T> Asset for $name<T>
            where
                T: for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
            {
                const EXTENSIONS: &'static [&'static str] = &[$( $ext ),*];
                type Loader = loader::LoadFrom<T, $loader>;
            }

            #[cfg(feature = $feature)]
            impl<T> AsRef<T> for $name<T> {
                #[inline]
                fn as_ref(&self) -> &T {
                    &self.0
                }
            }
        )*
    }
}

serde_assets! {
    #[cfg(feature = "json")]
    struct Json => (
        loader::JsonLoader,
        ["json"],
    );

    #[cfg(feature = "ron")]
    struct Ron => (
        loader::RonLoader,
        ["ron"],
    );
}