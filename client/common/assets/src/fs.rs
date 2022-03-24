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

    fn read_dir(&self, id: &str, f: &mut dyn FnMut(DirEntry)) -> io::Result<()> {

        let map = super::ASSET_MAP_DIR.lock().unwrap();
        for key in map.keys() {
            if key.starts_with(id) {
                f(DirEntry::Directory(key))
            }
        }

        let fileMap = super::ASSET_MAP.lock().unwrap();
        for (key, value) in fileMap.iter() {
            if key.starts_with(id) {
                if let Some(pos) = key.rfind(".") {
                    let name = &key[0..pos - 1];
                    let ext = &key[pos..];
                    f(DirEntry::File(name, ext))
                }
            }
        }
        io::Result::Ok(())
    }

    fn exists(&self, entry: DirEntry) -> bool { 

        //判断文件或者文件夹是否存在
        if let DirEntry::File(id, ext) = entry {
            let mut name = String::from(id);
            name.push_str(&".");
            name.push_str(ext);

            let map = super::ASSET_MAP.lock().unwrap();
            if map.contains_key(&name) {
                return true
            }

        } else if let DirEntry::Directory(dir) = entry {
            let map = super::ASSET_MAP_DIR.lock().unwrap();
            if map.contains_key(dir) {
                return true
            }
        }
        false
    }

    fn make_source(&self) -> Option<Box<dyn Source + Send>> { Some(Box::new(self.clone())) }
}
