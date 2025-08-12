use age::{Decryptor, Identity, IdentityFile};
use anyhow::{Result, anyhow};
use std::ffi::{CStr, CString};
use std::fs::{self, DirBuilder};
use std::io::{Read, Write};
use std::os::unix::fs::DirBuilderExt;
use std::os::{raw::c_char, unix::fs::OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::ptr;
use users::get_current_uid;

trait SecretCache {
    fn create(&self, path: &Path, content: &str) -> Result<()>;

    fn load(&self, path: &Path) -> Result<Option<String>>;

    fn load_or(&self, path: &Path, f: &dyn Fn(&Path) -> Result<String>) -> Result<String> {
        match self.load(path)? {
            Some(content) => Ok(content),
            None => {
                let content = f(path)?;
                self.create(path, &content)?;
                Ok(content)
            }
        }
    }
}

struct NullCache;

impl SecretCache for NullCache {
    fn create(&self, _path: &Path, _content: &str) -> Result<()> {
        Ok(())
    }

    fn load(&self, _path: &Path) -> Result<Option<String>> {
        Ok(None)
    }
}

struct TempCache {
    dir: PathBuf,
}

impl TempCache {
    fn get_identity(&self, path: &Path) -> Result<String> {
        path.to_str()
            .map(sha256::digest)
            .ok_or(anyhow!("Incorrect path!"))
    }

    fn get_base_cache_path(&self) -> std::path::PathBuf {
        let mut cache_path = self.dir.clone();
        cache_path.push("nix-rage-cache");
        cache_path
    }

    fn get_user_cache_path(&self) -> std::path::PathBuf {
        let mut cache_path = self.get_base_cache_path();
        cache_path.push(get_current_uid().to_string());
        cache_path
    }

    fn gen_cache_path(&self, path: &Path) -> Result<std::path::PathBuf> {
        let identity = self.get_identity(path)?;
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(anyhow!("Incorrect path!"))?;
        let mut cache_path = self.get_user_cache_path();
        cache_path.push(format!("{identity}-{file_name}"));
        Ok(cache_path)
    }
}

impl Default for TempCache {
    fn default() -> Self {
        Self {
            dir: tempfile::env::temp_dir(),
        }
    }
}

impl SecretCache for TempCache {
    fn create(&self, path: &Path, content: &str) -> Result<()> {
        let cache_path = self.gen_cache_path(path)?;
        let base_cache_path = self.get_base_cache_path();
        if !fs::exists(&base_cache_path)? {
            DirBuilder::new().mode(0o222).create(&base_cache_path)?;
        }
        let user_cache_path = self.get_user_cache_path();
        if !fs::exists(&user_cache_path)? {
            DirBuilder::new().mode(0o700).create(&user_cache_path)?;
        }
        let mut output = fs::OpenOptions::new()
            .truncate(true)
            .create(true)
            .write(true)
            .mode(0o700)
            .open(cache_path)?;
        write!(output, "{}", content)?;
        Ok(())
    }

    fn load(&self, path: &Path) -> Result<Option<String>> {
        let user_cache_path = self.gen_cache_path(path)?;
        Ok(if fs::exists(&user_cache_path)? {
            Some(fs::read_to_string(&user_cache_path)?)
        } else {
            None
        })
    }
}

fn decrypt_file<'a>(
    identities: impl Iterator<Item = &'a dyn Identity>,
    path: &'a Path,
) -> Result<String> {
    let file = fs::File::open(path)?;
    let decryptor = Decryptor::new(file)?;
    let mut stream = decryptor.decrypt(identities)?;
    let mut content = String::new();
    stream.read_to_string(&mut content)?;
    Ok(content)
}

fn _nix_rage_decrypt(identities: Vec<&str>, filename: &str, cache: bool) -> Result<String> {
    let identities = identities
        .into_iter()
        .map(&str::to_string)
        .flat_map(IdentityFile::from_file)
        .flat_map(IdentityFile::into_identities)
        .flatten()
        .collect::<Vec<_>>();
    let cache: Box<dyn SecretCache> = if cache {
        Box::new(TempCache::default())
    } else {
        Box::new(NullCache)
    };
    let content = cache.load_or(Path::new(filename), &|path| {
        decrypt_file(identities.iter().map(|b| &**b), path)
    })?;
    Ok(content)
}

static mut DECRYPT_ERROR: Option<String> = None;

fn set_error(err: String) {
    unsafe {
        DECRYPT_ERROR = Some(err);
    }
}

#[no_mangle]
#[allow(clippy::missing_safety_doc, static_mut_refs)]
pub unsafe extern "C" fn nix_rage_decrypt_error() -> *const c_char {
    unsafe {
        DECRYPT_ERROR
            .clone()
            .and_then(|err| CString::new(err).ok())
            .map(|err| err.into_raw() as *const c_char)
            .unwrap_or(ptr::null())
    }
}

#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn nix_rage_decrypt(
    identities: *const *const c_char,
    size: usize,
    filename: *const c_char,
    cache: bool,
) -> *const c_char {
    let fname = unsafe { CStr::from_ptr(filename) }.to_str();
    let identities = unsafe { std::slice::from_raw_parts(identities, size) }
        .iter()
        .map(|ptr| unsafe { CStr::from_ptr(*ptr) }.to_str())
        .collect::<Result<Vec<_>, _>>();
    identities
        .ok()
        .and_then(|i| {
            fname
                .ok()
                .map(|f| _nix_rage_decrypt(i, f, cache))
                .and_then(|r| r.map_err(|err| set_error(err.to_string())).ok())
        })
        .and_then(|content| CString::new(content).ok())
        .map(|content| content.into_raw() as *const c_char)
        .unwrap_or(ptr::null())
}
