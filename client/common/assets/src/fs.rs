use std::{borrow::Cow, io};
use assets_manager::{
    source::{DirEntry, Source},
};

/// Loads assets from the default path or `VELOREN_ASSETS_OVERRIDE` env if it is
/// set.
#[derive(Debug, Clone)]
pub struct ResSystem {}

impl ResSystem {
    pub fn new() -> io::Result<Self> { Ok(Self {}) }
}

impl Source for ResSystem {
    fn read(&self, id: &str, ext: &str) -> io::Result<Cow<[u8]>> {
        let result = super::get_cache_data(id, ext);
        match result {
            Ok(bytes) => Ok(bytes),
            Err(res_error) => {
                let error_msg = format!("load asset error:{:?}", res_error);
                let error = io::Error::new(io::ErrorKind::Other, error_msg);
                Err(error)
            }
        }
    }

    fn read_dir(&self, id: &str, _: &mut dyn FnMut(DirEntry)) -> io::Result<()> {
        use std::io::{Error, ErrorKind};
        let s = format!("读取文件夹 {}", id);
        let custom_error = Error::new(ErrorKind::Other, s);
        Err(custom_error)
    }

    fn exists(&self, entry: DirEntry) -> bool { 
        true 
    }

    fn make_source(&self) -> Option<Box<dyn Source + Send>> { Some(Box::new(self.clone())) }
}
