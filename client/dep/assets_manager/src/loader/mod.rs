use crate::{BoxedError, SharedBytes, SharedString};

use std::{
    borrow::Cow,
    marker::PhantomData,
    str::{self, FromStr},
};

pub trait Loader<T> {
    fn load(content: Cow<[u8]>, ext: &str) -> Result<T, BoxedError>;
}

#[derive(Debug)]
pub struct LoadFrom<U, L>(PhantomData<(U, L)>);
impl<T, U, L> Loader<T> for LoadFrom<U, L>
where
    U: Into<T>,
    L: Loader<U>,
{
    #[inline]
    fn load(content: Cow<[u8]>, ext: &str) -> Result<T, BoxedError> {
        Ok(L::load(content, ext)?.into())
    }
}

pub type LoadFromAsset<A> = LoadFrom<A, <A as crate::Asset>::Loader>;

#[derive(Debug)]
pub struct BytesLoader(());
impl Loader<Vec<u8>> for BytesLoader {
    #[inline]
    fn load(content: Cow<[u8]>, _: &str) -> Result<Vec<u8>, BoxedError> {
        Ok(content.into_owned())
    }
}
impl Loader<Box<[u8]>> for BytesLoader {
    #[inline]
    fn load(content: Cow<[u8]>, _: &str) -> Result<Box<[u8]>, BoxedError> {
        Ok(content.into())
    }
}
impl Loader<SharedBytes> for BytesLoader {
    #[inline]
    fn load(content: Cow<[u8]>, _: &str) -> Result<SharedBytes, BoxedError> {
        Ok(content.into())
    }
}

#[derive(Debug)]
pub struct StringLoader(());
impl Loader<String> for StringLoader {
    #[inline]
    fn load(content: Cow<[u8]>, _: &str) -> Result<String, BoxedError> {
        Ok(String::from_utf8(content.into_owned())?)
    }
}
impl Loader<Box<str>> for StringLoader {
    #[inline]
    fn load(content: Cow<[u8]>, ext: &str) -> Result<Box<str>, BoxedError> {
        StringLoader::load(content, ext).map(String::into_boxed_str)
    }
}
impl Loader<SharedString> for StringLoader {
    #[inline]
    fn load(content: Cow<[u8]>, _: &str) -> Result<SharedString, BoxedError> {
        Ok(match content {
            Cow::Owned(o) => String::from_utf8(o)?.into(),
            Cow::Borrowed(b) => str::from_utf8(b)?.into(),
        })
    }
}

#[derive(Debug)]
pub struct ParseLoader(());
impl<T> Loader<T> for ParseLoader
where
    T: FromStr,
    BoxedError: From<<T as FromStr>::Err>,
{
    #[inline]
    fn load(content: Cow<[u8]>, _: &str) -> Result<T, BoxedError> {
        Ok(str::from_utf8(&content)?.trim().parse()?)
    }
}

/// Loads fonts.
#[derive(Debug)]
pub struct FontLoader(());

macro_rules! serde_loaders {
    (
        $(
            #[cfg(feature = $feature:literal)]
            struct $name:ident => $fun:path;
        )*
    ) => {
        $(
            #[cfg(feature = $feature)]
            #[cfg_attr(docsrs, doc(cfg(feature = $feature)))]
            #[derive(Debug)]
            pub struct $name(());

            #[cfg(feature = $feature)]
            #[cfg_attr(docsrs, doc(cfg(feature = $feature)))]
            impl<T> Loader<T> for $name
            where
                T: for<'de> serde::Deserialize<'de>,
            {
                #[inline]
                fn load(content: Cow<[u8]>, _: &str) -> Result<T, BoxedError> {
                    Ok($fun(&*content)?)
                }
            }
        )*
    }
}

serde_loaders! {
    #[cfg(feature = "bincode")]
    struct BincodeLoader => serde_bincode::deserialize;

    #[cfg(feature = "json")]
    struct JsonLoader => serde_json::from_slice;


    #[cfg(feature = "ron")]
    struct RonLoader => serde_ron::de::from_bytes;
}
