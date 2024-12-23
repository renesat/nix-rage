use age::{Decryptor, Identity, IdentityFile};
use anyhow::Result;
use std::ffi::{CStr, CString};
use std::fs;
use std::io::Read;
use std::os::raw::c_char;
use std::path::Path;
use std::ptr;

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

fn _nix_rage_decrypt(identities: Vec<&str>, filename: &str) -> Result<String> {
    let identities = identities
        .into_iter()
        .map(&str::to_string)
        .flat_map(IdentityFile::from_file)
        .flat_map(IdentityFile::into_identities)
        .flatten()
        .collect::<Vec<_>>();
    let filename = Path::new(filename);
    decrypt_file(identities.iter().map(|b| &**b), filename)
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
                .map(|f| _nix_rage_decrypt(i, f))
                .and_then(|r| r.map_err(|err| set_error(err.to_string())).ok())
        })
        .and_then(|content| CString::new(content).ok())
        .map(|content| content.into_raw() as *const c_char)
        .unwrap_or(ptr::null())
}
