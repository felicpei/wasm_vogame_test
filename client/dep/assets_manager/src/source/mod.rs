use std::{borrow::Cow, io};

/// An entry in a source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirEntry<'a> {
    /// A file with an id and an extension.
    File(&'a str, &'a str),

    /// A directory with an id.
    Directory(&'a str),
}

impl<'a> DirEntry<'a> {
    /// Returns `true` if this is a `File`.
    #[inline]
    pub const fn is_file(&self) -> bool {
        matches!(self, DirEntry::File(..))
    }

    /// Returns `true` if this is a `Directory`.
    #[inline]
    pub const fn is_dir(&self) -> bool {
        matches!(self, DirEntry::Directory(_))
    }

    /// Returns the id of the pointed entity.
    #[inline]
    pub const fn id(self) -> &'a str {
        match self {
            DirEntry::File(id, _) => id,
            DirEntry::Directory(id) => id,
        }
    }

    
    #[inline]
    pub fn parent_id(self) -> Option<&'a str> {
        let id = self.id();
        if id.is_empty() {
            None
        } else {
            match id.rfind('.') {
                Some(n) => Some(&id[..n]),
                None => Some(""),
            }
        }
    }
}

pub trait Source {
   
    fn read(&self, id: &str, ext: &str) -> io::Result<Cow<[u8]>>;

    fn read_dir(&self, id: &str, f: &mut dyn FnMut(DirEntry)) -> io::Result<()>;

    fn exists(&self, entry: DirEntry) -> bool;

    #[inline]
    fn make_source(&self) -> Option<Box<dyn Source + Send>> {
        None
    }
}


/// A [`Source`] that contains nothing.
///
/// Calling `read` or `read_dir` from this source will always return an error.
#[derive(Debug)]
pub struct Empty;

impl Source for Empty {
    #[inline]
    fn read(&self, _id: &str, _ext: &str) -> io::Result<Cow<[u8]>> {
        Err(io::Error::from(io::ErrorKind::NotFound))
    }

    #[inline]
    fn read_dir(&self, _id: &str, _f: &mut dyn FnMut(DirEntry)) -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::NotFound))
    }

    #[inline]
    fn exists(&self, _entry: DirEntry) -> bool {
        false
    }
}
