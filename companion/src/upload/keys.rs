//! The install's secrets: an Ed25519 seed and the 32-byte name key. They live in the
//! Windows credential store and nowhere else: never in a file, never printed or logged.

use super::{Error, Result};

/// Never `Debug`: the bytes must not reach a log line.
pub struct Identity {
    pub seed: [u8; 32],
    pub name_key: [u8; 32],
}

impl Identity {
    fn random() -> Result<Identity> {
        let mut b = [0u8; 64];
        getrandom::fill(&mut b).map_err(|e| Error::local(format!("no random numbers: {e}")))?;
        let mut seed = [0u8; 32];
        let mut name_key = [0u8; 32];
        seed.copy_from_slice(&b[..32]);
        name_key.copy_from_slice(&b[32..]);
        Ok(Identity { seed, name_key })
    }

    #[cfg(windows)]
    fn to_blob(&self) -> [u8; 64] {
        let mut b = [0u8; 64];
        b[..32].copy_from_slice(&self.seed);
        b[32..].copy_from_slice(&self.name_key);
        b
    }

    #[cfg(windows)]
    fn from_blob(b: &[u8]) -> Option<Identity> {
        if b.len() != 64 {
            return None;
        }
        let mut seed = [0u8; 32];
        let mut name_key = [0u8; 32];
        seed.copy_from_slice(&b[..32]);
        name_key.copy_from_slice(&b[32..]);
        Some(Identity { seed, name_key })
    }
}

/// Where the secrets are kept.
pub trait KeyStore {
    fn load(&self) -> Result<Option<Identity>>;
    fn save(&self, id: &Identity) -> Result<()>;
    fn delete(&self) -> Result<()>;
}

/// The identity in the store, made on first use when `create` is set. The flag is true when it
/// was made just now, so install ids remembered for an earlier key are stale.
pub fn identity(store: &dyn KeyStore, create: bool) -> Result<Option<(Identity, bool)>> {
    if let Some(id) = store.load()? {
        return Ok(Some((id, false)));
    }
    if !create {
        return Ok(None);
    }
    let id = Identity::random()?;
    store.save(&id)?;
    Ok(Some((id, true)))
}

/// The store of this platform.
pub fn system_store() -> Box<dyn KeyStore> {
    #[cfg(windows)]
    {
        Box::new(windows::CredentialManager::new())
    }
    #[cfg(not(windows))]
    {
        Box::new(Unavailable)
    }
}

/// A store that keeps nothing, for platforms without a credential store: uploading is not
/// available there.
#[cfg(not(windows))]
pub struct Unavailable;

#[cfg(not(windows))]
impl KeyStore for Unavailable {
    fn load(&self) -> Result<Option<Identity>> {
        Err(not_available())
    }
    fn save(&self, _: &Identity) -> Result<()> {
        Err(not_available())
    }
    fn delete(&self) -> Result<()> {
        Err(not_available())
    }
}

#[cfg(not(windows))]
fn not_available() -> Error {
    Error::local(
        "Uploading is not available on this system: it needs the Windows credential store.",
    )
}

#[cfg(windows)]
mod windows {
    use super::{Error, Identity, KeyStore, Result};
    use windows_sys::Win32::Foundation::{ERROR_NOT_FOUND, GetLastError};
    use windows_sys::Win32::Security::Credentials::{
        CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree,
        CredReadW, CredWriteW,
    };

    /// Credential Manager, one generic credential named `Scarlet upload key`
    /// (`SCARLET_UPLOAD_CRED` names another, to try a fresh install identity).
    pub struct CredentialManager {
        target: Vec<u16>,
    }

    impl CredentialManager {
        pub fn new() -> CredentialManager {
            let name = std::env::var("SCARLET_UPLOAD_CRED")
                .ok()
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| "Scarlet upload key".into());
            CredentialManager {
                target: name.encode_utf16().chain(std::iter::once(0)).collect(),
            }
        }
    }

    fn fail(what: &str) -> Error {
        // SAFETY: reads the calling thread's last error.
        let code = unsafe { GetLastError() };
        Error::local(format!(
            "The Windows credential store refused to {what} (error {code})."
        ))
    }

    impl KeyStore for CredentialManager {
        fn load(&self) -> Result<Option<Identity>> {
            let mut p: *mut CREDENTIALW = std::ptr::null_mut();
            // SAFETY: on success `p` points to a credential the store allocated; it is freed below.
            let ok = unsafe { CredReadW(self.target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut p) };
            if ok == 0 {
                // SAFETY: reads the calling thread's last error.
                if unsafe { GetLastError() } == ERROR_NOT_FOUND {
                    return Ok(None);
                }
                return Err(fail("read the install key"));
            }
            // SAFETY: `p` is valid until CredFree; the blob is `CredentialBlobSize` bytes.
            let id = unsafe {
                let c = &*p;
                let blob = if c.CredentialBlob.is_null() {
                    &[][..]
                } else {
                    std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize)
                };
                let id = Identity::from_blob(blob);
                CredFree(p.cast());
                id
            };
            match id {
                Some(id) => Ok(Some(id)),
                None => Err(Error::local(
                    "The install key in the Windows credential store is damaged.",
                )),
            }
        }

        fn save(&self, id: &Identity) -> Result<()> {
            let mut blob = id.to_blob();
            let mut user: Vec<u16> = "scarlet".encode_utf16().chain(std::iter::once(0)).collect();
            // SAFETY: a zeroed CREDENTIALW is valid; the pointers outlive the call.
            let mut c: CREDENTIALW = unsafe { std::mem::zeroed() };
            c.Type = CRED_TYPE_GENERIC;
            c.TargetName = self.target.as_ptr().cast_mut();
            c.CredentialBlobSize = blob.len() as u32;
            c.CredentialBlob = blob.as_mut_ptr();
            c.Persist = CRED_PERSIST_LOCAL_MACHINE;
            c.UserName = user.as_mut_ptr();
            // SAFETY: `c` and what it points to are alive for the call.
            let ok = unsafe { CredWriteW(&c, 0) };
            blob.fill(0);
            if ok == 0 {
                return Err(fail("save the install key"));
            }
            Ok(())
        }

        fn delete(&self) -> Result<()> {
            // SAFETY: plain call with a NUL-terminated name.
            let ok = unsafe { CredDeleteW(self.target.as_ptr(), CRED_TYPE_GENERIC, 0) };
            // SAFETY: reads the calling thread's last error.
            if ok == 0 && unsafe { GetLastError() } != ERROR_NOT_FOUND {
                return Err(fail("delete the install key"));
            }
            Ok(())
        }
    }
}
