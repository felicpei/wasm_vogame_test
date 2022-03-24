//! Definition of the cache

use crate::{
    asset::{DirLoadable, Storable},
    dirs::DirHandle,
    entry::{CacheEntry, CacheEntryInner},
    error::ErrorKind,
    loader::Loader,
    source::{Empty, Source},
    utils::{BorrowedKey, HashMap, Key, OwnedKey, Private, RandomState, RwLock},
    Asset, Compound, Error, Handle, SharedString,
};

use std::{any::TypeId, fmt};

#[repr(align(64))]
struct Shard(RwLock<HashMap<OwnedKey, CacheEntry>>);

pub(crate) struct Map {
    hash_builder: RandomState,
    shards: Box<[Shard]>,
}

impl Map {
    fn new(min_shards: usize) -> Map {
        let shards = min_shards.next_power_of_two();

        let hash_builder = RandomState::new();
        let shards = (0..shards)
            .map(|_| Shard(RwLock::new(HashMap::with_hasher(hash_builder.clone()))))
            .collect();

        Map {
            hash_builder,
            shards,
        }
    }

    fn get_shard(&self, key: BorrowedKey) -> &Shard {
        use std::hash::*;

        let mut hasher = self.hash_builder.build_hasher();
        key.hash(&mut hasher);
        let id = (hasher.finish() as usize) & (self.shards.len() - 1);
        &self.shards[id]
    }

    fn get_shard_mut(&mut self, key: BorrowedKey) -> &mut Shard {
        use std::hash::*;

        let mut hasher = self.hash_builder.build_hasher();
        key.hash(&mut hasher);
        let id = (hasher.finish() as usize) & (self.shards.len() - 1);
        &mut self.shards[id]
    }

    pub fn get_entry(&self, key: BorrowedKey) -> Option<CacheEntryInner> {
        let shard = self.get_shard(key).0.read();
        let entry = shard.get(&key as &dyn Key)?;
        unsafe { Some(entry.inner().extend_lifetime()) }
    }

    fn insert(&self, key: OwnedKey, entry: CacheEntry) -> CacheEntryInner {
        let shard = &mut *self.get_shard(key.borrow()).0.write();
        let entry = shard.entry(key).or_insert(entry);
        unsafe { entry.inner().extend_lifetime() }
    }

    fn contains_key(&self, key: BorrowedKey) -> bool {
        let shard = self.get_shard(key).0.read();
        shard.contains_key(&key as &dyn Key)
    }

    fn take(&mut self, key: BorrowedKey) -> Option<CacheEntry> {
        self.get_shard_mut(key).0.get_mut().remove(&key as &dyn Key)
    }

    #[inline]
    fn remove(&mut self, key: BorrowedKey) -> bool {
        self.take(key).is_some()
    }

    fn clear(&mut self) {
        for shard in &mut *self.shards {
            shard.0.get_mut().clear();
        }
    }
}

impl fmt::Debug for Map {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut map = f.debug_map();

        for shard in &*self.shards {
            map.entries(&**shard.0.read());
        }

        map.finish()
    }
}

pub struct AssetCache<S: ?Sized = Empty> {
    pub(crate) assets: Map,
    source: S,
}

impl<S: Source> AssetCache<S> {
    pub fn with_source(source: S) -> AssetCache<S> {
        AssetCache {
            assets: Map::new(32),
            source,
        }
    }
}

impl<S> AssetCache<S> {
    /// Creates a cache that loads assets from the given source.
    pub fn without_hot_reloading(source: S) -> AssetCache<S> {
        AssetCache {
            assets: Map::new(32),
            source,
        }
    }
}

impl<S> AssetCache<S>
where
    S: ?Sized,
{
    /// Returns a reference to the cache's [`Source`].
    #[inline]
    pub fn source(&self) -> &S {
        &self.source
    }

    #[inline]
    pub fn no_record<T, F: FnOnce() -> T>(&self, f: F) -> T {
        f()
    }

    /// Adds an asset to the cache.
    ///
    /// This function does not not have the asset kind as generic parameter to
    /// reduce monomorphisation.
    #[cold]
    fn add_asset(
        &self,
        id: &str,
        type_id: TypeId,
        load: fn(&Self, SharedString) -> Result<CacheEntry, Error>,
    ) -> Result<CacheEntryInner, Error> {
        log::trace!("Loading \"{}\"", id);

        let id = SharedString::from(id);
        let entry = load(self, id.clone())?;
        let key = OwnedKey::new_with(id, type_id);

        Ok(self.assets.insert(key, entry))
    }

    /// Adds any value to the cache.
    #[cold]
    fn add_any<A: Storable>(&self, id: &str, asset: A) -> CacheEntryInner {
        let id = SharedString::from(id);
        let entry = CacheEntry::new(asset, id.clone());
        let key = OwnedKey::new::<A>(id);

        self.assets.insert(key, entry)
    }

    #[inline]
    pub fn get_cached<A: Storable>(&self, id: &str) -> Option<Handle<A>> {
        Some(self.get_cached_entry::<A>(id)?.handle())
    }

    #[inline]
    fn get_cached_entry<A: Storable>(&self, id: &str) -> Option<CacheEntryInner> {
        self.get_cached_entry_inner(id, TypeId::of::<A>())
    }

    fn get_cached_entry_inner(&self, id: &str, type_id: TypeId) -> Option<CacheEntryInner> {
        let key = BorrowedKey::new_with(id, type_id);
        self.assets.get_entry(key)
    }

    #[inline]
    pub fn get_or_insert<A: Storable>(&self, id: &str, default: A) -> Handle<A> {
        let entry = match self.get_cached_entry::<A>(id) {
            Some(entry) => entry,
            None => self.add_any(id, default),
        };

        entry.handle()
    }

    /// Returns `true` if the cache contains the specified asset.
    #[inline]
    pub fn contains<A: Storable>(&self, id: &str) -> bool {
        let key = BorrowedKey::new::<A>(id);
        self.assets.contains_key(key)
    }

    #[inline]
    pub fn get_cached_dir<A: DirLoadable>(
        &self,
        id: &str,
        recursive: bool,
    ) -> Option<DirHandle<A, S>> {
        Some(if recursive {
            let handle = self.get_cached(id)?;
            DirHandle::new_rec(handle, self)
        } else {
            let handle = self.get_cached(id)?;
            DirHandle::new(handle, self)
        })
    }

    /// Returns `true` if the cache contains the specified directory with the
    /// given `recursive` parameter.
    #[inline]
    pub fn contains_dir<A: DirLoadable>(&self, id: &str, recursive: bool) -> bool {
        self.get_cached_dir::<A>(id, recursive).is_some()
    }

    /// Removes an asset from the cache, and returns whether it was present in
    /// the cache.
    ///
    /// Note that you need a mutable reference to the cache, so you cannot have
    /// any [`Handle`], [`AssetGuard`], etc when you call this function.
    #[inline]
    pub fn remove<A: Storable>(&mut self, id: &str) -> bool {
        let key = BorrowedKey::new::<A>(id);
        let removed = self.assets.remove(key);
        removed
    }

    /// Takes ownership on a cached asset.
    ///
    /// The corresponding asset is removed from the cache.
    #[inline]
    pub fn take<A: Storable>(&mut self, id: &str) -> Option<A> {
        let key = BorrowedKey::new::<A>(id);
        self.assets.take(key).map(|e| {
            let (asset, _id) = e.into_inner();

            asset
        })
    }

    /// Clears the cache.
    ///
    /// Removes all cached assets and directories.
    #[inline]
    pub fn clear(&mut self) {
        self.assets.clear();
    }
}

impl<S> AssetCache<S>
where
    S: Source + ?Sized,
{
    #[inline]
    pub fn load<A: Compound>(&self, id: &str) -> Result<Handle<A>, Error> {
        let entry = match self.get_cached_entry::<A>(id) {
            Some(entry) => entry,
            None => {
                let load = A::_load_and_record_entry::<S, Private>;
                let type_id = TypeId::of::<A>();
                self.add_asset(id, type_id, load)?
            }
        };

        Ok(entry.handle())
    }

    #[inline]
    #[track_caller]
    pub fn load_expect<A: Compound>(&self, id: &str) -> Handle<A> {
        #[cold]
        #[track_caller]
        fn expect_failed(err: Error) -> ! {
            panic!(
                "Failed to load essential asset \"{}\": {}",
                err.id(),
                err.reason()
            )
        }

        // Do not use `unwrap_or_else` as closures do not have #[track_caller]
        match self.load(id) {
            Ok(h) => h,
            Err(err) => expect_failed(err),
        }
    }

    /// ignored.
    #[inline]
    pub fn load_dir<A: DirLoadable>(
        &self,
        id: &str,
        recursive: bool,
    ) -> Result<DirHandle<A, S>, Error> {
        Ok(if recursive {
            let handle = self.load(id)?;
            DirHandle::new_rec(handle, self)
        } else {
            let handle = self.load(id)?;
            DirHandle::new(handle, self)
        })
    }

    #[inline]
    pub fn load_owned<A: Compound>(&self, id: &str) -> Result<A, Error> {
        let id = SharedString::from(id);
        let asset = A::_load_and_record::<S, Private>(self, &id);

        asset
    }
}

impl<S> AssetCache<S> where S: Source + Sync {}

impl<S> fmt::Debug for AssetCache<S>
where
    S: ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetCache")
            .field("assets", &self.assets)
            .finish()
    }
}

#[inline]
fn load_single<A, S>(source: &S, id: &str, ext: &str) -> Result<A, ErrorKind>
where
    A: Asset,
    S: Source + ?Sized,
{
    let content = source.read(id, ext)?;
    let asset = A::Loader::load(content, ext)?;
    Ok(asset)
}

pub(crate) fn load_from_source<A, S>(source: &S, id: &str) -> Result<A, Error>
where
    A: Asset,
    S: Source + ?Sized,
{
    let mut error = ErrorKind::NoDefaultValue;

    for ext in A::EXTENSIONS {
        match load_single(source, id, ext) {
            Err(err) => error = err.or(error),
            Ok(asset) => return Ok(asset),
        }
    }

    A::default_value(id, Error::from_kind(id.into(), error))
}
