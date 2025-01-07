use age::{Decryptor, Identity, IdentityFile};
use anyhow::Result;
use sha256::digest;
use std::ffi::{CStr, CString};
use std::fs;
use std::io::{Read, Write};
use std::os::{
    raw::c_char,
    unix::fs::{OpenOptionsExt, PermissionsExt},
};
use std::path::Path;
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

fn get_base_cache_path() -> std::path::PathBuf {
    let mut cache_path = tempfile::env::temp_dir();
    cache_path.push("nix-rage-cache");
    cache_path
}

fn get_user_cache_path() -> std::path::PathBuf {
    let mut cache_path = get_base_cache_path();
    cache_path.push(get_current_uid().to_string());
    cache_path
}

fn gen_cache_path(path: &Path) -> std::path::PathBuf {
    let cache_filename = path
        .to_str()
        .map(digest)
        .and_then(|f| {
            path.file_name()
                .and_then(|n| n.to_str())
                .map(|n| format!("{f}-{n}"))
        })
        .expect("Incorrect file path!");
    let mut cache_path = get_user_cache_path();
    cache_path.push(cache_filename);
    cache_path
}

fn create_cache(path: &Path, content: &str) -> Result<()> {
    let cache_path = gen_cache_path(path);
    let base_cache_path = get_base_cache_path();
    if !fs::exists(&base_cache_path)? {
        fs::create_dir_all(&base_cache_path)?;
        let mut perms = fs::metadata(&base_cache_path)?.permissions();
        perms.set_mode(0o777);
        fs::set_permissions(&base_cache_path, perms)?;
    }
    let user_cache_path = get_user_cache_path();
    if !fs::exists(&user_cache_path)? {
        fs::create_dir_all(&user_cache_path)?;
        let mut perms = fs::metadata(&user_cache_path)?.permissions();
        perms.set_mode(0o700);
        fs::set_permissions(&user_cache_path, perms)?;
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

fn _nix_rage_decrypt(identities: Vec<&str>, filename: &str, cache: bool) -> Result<String> {
    let identities = identities
        .into_iter()
        .map(&str::to_string)
        .flat_map(IdentityFile::from_file)
        .flat_map(IdentityFile::into_identities)
        .flatten()
        .collect::<Vec<_>>();
    let filename = Path::new(filename);
    if cache {
        let user_cache_path = gen_cache_path(filename);
        if fs::exists(&user_cache_path)? {
            return Ok(fs::read_to_string(&user_cache_path)?);
        }
    }
    let content = decrypt_file(identities.iter().map(|b| &**b), filename)?;
    if cache {
        create_cache(filename, &content)?;
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
