use age::{Decryptor, Identity, IdentityFile};
use anyhow::Result;
use sha256::digest;
use std::ffi::{CStr, CString};
use std::fs;
use std::io::{Read, Write};
use std::ops::Not;
use std::os::{
    raw::c_char,
    unix::fs::{OpenOptionsExt, PermissionsExt},
};
use std::path::{Path, PathBuf};
use std::ptr;
use users::get_current_uid;

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

#[derive(Debug)]
struct DecryptCache {
    cache_dir: PathBuf,
}

impl Default for DecryptCache {
    fn default() -> Self {
        let mut cache_dir = tempfile::env::temp_dir();
        cache_dir.push("nix-rage-cache");
        Self { cache_dir }
    }
}

impl DecryptCache {
    fn new<P: AsRef<Path>>(cache_dir: P) -> Self {
        Self {
            cache_dir: cache_dir.as_ref().to_path_buf(),
        }
    }

    fn user_cache_path(&self, uid: u32) -> std::path::PathBuf {
        let mut cache_path = self.cache_dir.clone();
        cache_path.push(uid.to_string());
        cache_path
    }

    fn make_user_cache_path(&self, uid: u32) -> Result<()> {
        let base_cache_path = &self.cache_dir;
        if !fs::exists(base_cache_path)? {
            fs::create_dir_all(base_cache_path)?;
            let mut perms = fs::metadata(base_cache_path)?.permissions();
            perms.set_mode(0o777);
            fs::set_permissions(base_cache_path, perms)?;
        }
        let user_cache_path = self.user_cache_path(uid);
        if !fs::exists(&user_cache_path)? {
            fs::create_dir_all(&user_cache_path)?;
            let mut perms = fs::metadata(&user_cache_path)?.permissions();
            perms.set_mode(0o700);
            fs::set_permissions(&user_cache_path, perms)?;
        }
        Ok(())
    }

    fn gen_cache_path<P: AsRef<Path>>(&self, path: P, uid: u32) -> std::path::PathBuf {
        let cache_filename = path
            .as_ref()
            .to_str()
            .map(digest)
            .and_then(|f| {
                path.as_ref()
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| format!("{f}-{n}"))
            })
            .expect("Incorrect file path!");
        let mut cache_path = self.user_cache_path(uid);
        cache_path.push(cache_filename);
        cache_path
    }

    fn cache<P: AsRef<Path>>(&self, path: P, content: &str) -> Result<()> {
        let uid = get_current_uid();
        let cache_path = self.gen_cache_path(path, uid);
        self.make_user_cache_path(uid)?;
        let mut output = fs::OpenOptions::new()
            .truncate(true)
            .create(true)
            .write(true)
            .mode(0o700)
            .open(cache_path)?;
        write!(output, "{}", content)?;
        Ok(())
    }

    fn load<P: AsRef<Path>>(&self, path: P) -> Result<Option<String>> {
        let uid = get_current_uid();
        let cache_path = self.gen_cache_path(path, uid);
        if fs::exists(&cache_path)? {
            return Ok(Some(fs::read_to_string(&cache_path)?));
        }
        Ok(None)
    }
}

fn _nix_rage_decrypt(
    identities: Vec<&str>,
    filename: &str,
    cache: Option<DecryptCache>,
) -> Result<String> {
    let identities = identities
        .into_iter()
        .map(&str::to_string)
        .flat_map(IdentityFile::from_file)
        .flat_map(IdentityFile::into_identities)
        .flatten()
        .collect::<Vec<_>>();
    let filename = Path::new(filename);
    if let Some(cache) = &cache {
        if let Some(content) = cache.load(filename)? {
            return Ok(content);
        }
    }
    let content = decrypt_file(identities.iter().map(|b| &**b), filename)?;
    if let Some(cache) = &cache {
        cache.cache(filename, &content)?;
    }
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
    cache_dir: *const c_char,
) -> *const c_char {
    let fname = unsafe { CStr::from_ptr(filename) }.to_str().ok();
    let cache_dir = cache_dir
        .is_null()
        .not()
        .then(|| unsafe { CStr::from_ptr(cache_dir) })
        .map(CStr::to_str)
        .and_then(Result::ok);
    let identities = unsafe { std::slice::from_raw_parts(identities, size) }
        .iter()
        .map(|ptr| unsafe { CStr::from_ptr(*ptr) }.to_str())
        .collect::<Result<Vec<_>, _>>()
        .ok();
    identities
        .and_then(|i| fname.map(|f| (i, f)))
        .map(|(i, f)| {
            (
                i,
                f,
                cache.then(|| cache_dir.map(DecryptCache::new).unwrap_or_default()),
            )
        })
        .and_then(|(i, f, c)| {
            _nix_rage_decrypt(i, f, c)
                .map_err(|err| set_error(err.to_string()))
                .ok()
        })
        .and_then(|content| CString::new(content).ok())
        .map(|content| content.into_raw() as *const c_char)
        .unwrap_or(ptr::null())
}
