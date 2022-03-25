//! Definitions of cache entries

use std::{
    any::{Any},
    fmt,
    ops::Deref,
};

use crate::{
    asset::{NotHotReloaded, Storable},
    SharedString,
};

/// The representation of an asset whose value cannot change.
pub(crate) struct StaticInner<T> {
    id: SharedString,
    value: T,
}

impl<T> StaticInner<T> {
    #[inline]
    fn new(value: T, id: SharedString) -> Self {
        Self { id, value }
    }
}


#[derive(Clone, Copy)]
pub(crate) struct CacheEntryInner<'a>(&'a (dyn Any + Send + Sync));

impl<'a> CacheEntryInner<'a> {
    #[inline]
    pub unsafe fn extend_lifetime<'b>(self) -> CacheEntryInner<'b> {
        let inner = &*(self.0 as *const (dyn Any + Send + Sync));
        CacheEntryInner(inner)
    }

    #[inline]
    pub fn handle<T: 'static>(self) -> Handle<'a, T> {
        Handle::new(self)
    }
}

/// An entry in the cache.
pub struct CacheEntry(pub Box<dyn Any + Send + Sync>);

impl CacheEntry {
    /// Creates a new `CacheEntry` containing an asset of type `T`.
    ///
    /// The returned structure can safely use its methods with type parameter `T`.
    #[inline]
    pub fn new<T: Storable>(asset: T, id: SharedString) -> Self {
        let inner = Box::new(StaticInner::new(asset, id));
        CacheEntry(inner)
    }

    /// Returns a reference on the inner storage of the entry.
    #[inline]
    pub(crate) fn inner(&self) -> CacheEntryInner {
        CacheEntryInner(self.0.as_ref())
    }

    /// Consumes the `CacheEntry` and returns its inner value.
    #[inline]
    pub fn into_inner<T: 'static>(self) -> (T, SharedString) {
        let _this = match self.0.downcast::<StaticInner<T>>() {
            Ok(inner) => return (inner.value, inner.id),
            Err(this) => this,
        };

        wrong_handle_type()
    }
}

impl fmt::Debug for CacheEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CacheEntry").finish()
    }
}

enum HandleInner<'a, T> {
    Static(&'a StaticInner<T>),
}

impl<T> Clone for HandleInner<'_, T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for HandleInner<'_, T> {}

pub struct Handle<'a, T> {
    inner: HandleInner<'a, T>,
}

impl<'a, T> Handle<'a, T> {
    fn new(inner: CacheEntryInner<'a>) -> Self
    where
        T: 'static,
    {
        let inner = loop {
            if let Some(inner) = inner.0.downcast_ref::<StaticInner<T>>() {
                break HandleInner::Static(inner);
            }

            wrong_handle_type()
        };

        Handle { inner }
    }

    #[inline]
    fn either<U>(
        &self,
        on_static: impl FnOnce(&'a StaticInner<T>) -> U,
    ) -> U {
        match self.inner {
            HandleInner::Static(s) => on_static(s),
        }
    }

    #[inline]
    pub fn read(&self) -> AssetGuard<'a, T> {
        let inner = match self.inner {
            HandleInner::Static(this) => GuardInner::Ref(&this.value),
        };
        AssetGuard { inner }
    }

    /// Returns the id of the asset.
    ///
    /// Note that the lifetime of the returned `&str` is tied to that of the
    /// `AssetCache`, so it can outlive the handle.
    #[inline]
    pub fn id(&self) -> &'a str {
        self.either(|s| &s.id)
    }
}

impl<A> Handle<'_, A>
where
    A: Copy,
{
    /// Returns a copy of the inner asset.
    ///
    /// This is functionnally equivalent to `cloned`, but it ensures that no
    /// expensive operation is used (eg if a type is refactored).
    #[inline]
    pub fn copied(self) -> A {
        *self.read()
    }
}

impl<A> Handle<'_, A>
where
    A: Clone,
{
    /// Returns a clone of the inner asset.
    #[inline]
    pub fn cloned(self) -> A {
        self.read().clone()
    }
}

impl<'a, A> Handle<'a, A>
where
    A: NotHotReloaded,
{
    /// Returns a reference to the underlying asset.
    ///
    /// This method only works if hot-reloading is disabled for the given type.
    #[inline]
    pub fn get(&self) -> &'a A {
        self.either(
            |this| &this.value,
        )
    }
}

impl<A> Clone for Handle<'_, A> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<A> Copy for Handle<'_, A> {}

impl<T, U> PartialEq<Handle<'_, U>> for Handle<'_, T>
where
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &Handle<U>) -> bool {
        self.read().eq(&other.read())
    }
}

impl<A> Eq for Handle<'_, A> where A: Eq {}

#[cfg(feature = "serde")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
impl<A> serde::Serialize for Handle<'_, A>
where
    A: serde::Serialize,
{
    #[inline]
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.read().serialize(s)
    }
}

impl<A> fmt::Debug for Handle<'_, A>
where
    A: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Handle")
            .field("value", &*self.read())
            .finish()
    }
}

pub enum GuardInner<'a, T> {
    Ref(&'a T),
}

/// RAII guard used to keep a read lock on an asset and release it when dropped.
///
/// This type is a smart pointer to type `A`.
///
/// It can be obtained by calling [`Handle::read`].
pub struct AssetGuard<'a, A> {
    inner: GuardInner<'a, A>,
}

impl<A> Deref for AssetGuard<'_, A> {
    type Target = A;

    #[inline]
    fn deref(&self) -> &A {
        match &self.inner {
            GuardInner::Ref(r) => r,
        }
    }
}

impl<A, U> AsRef<U> for AssetGuard<'_, A>
where
    A: AsRef<U>,
{
    #[inline]
    fn as_ref(&self) -> &U {
        (&**self).as_ref()
    }
}

impl<A> fmt::Display for AssetGuard<'_, A>
where
    A: fmt::Display,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<A> fmt::Debug for AssetGuard<'_, A>
where
    A: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}


#[cold]
#[track_caller]
fn wrong_handle_type() -> ! {
    panic!("wrong handle type");
}
