pub mod terminal;

use russh::keys::{decode_secret_key, PrivateKeyWithHashAlg};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};

const SSH_KEYRING_SERVICE: &str = "com.paragdulam.remotedevmachine.ssh";
const VNC_KEYRING_SERVICE: &str = "com.paragdulam.remotedevmachine.vnc";
const PASSWORD_CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";

/// A random per-rental VNC password. Never written to the rentals file — the
/// in-memory record holds it, and the keyring copy (see `store_vnc_password`)
/// lets a restarted app reattach to a still-running rental.
pub fn generate_vnc_password() -> String {
    (0..12)
        .map(|_| {
            let idx = rand::random_range(0..PASSWORD_CHARS.len());
            PASSWORD_CHARS[idx] as char
        })
        .collect()
}

pub fn store_private_key(rental_id: &str, private_key_pem: &str) -> Result<(), String> {
    keyring::Entry::new(SSH_KEYRING_SERVICE, rental_id)
        .map_err(|e| e.to_string())?
        .set_password(private_key_pem)
        .map_err(|e| e.to_string())
}

pub fn load_private_key(rental_id: &str) -> Result<String, String> {
    keyring::Entry::new(SSH_KEYRING_SERVICE, rental_id)
        .map_err(|e| e.to_string())?
        .get_password()
        .map_err(|e| e.to_string())
}

pub fn store_vnc_password(rental_id: &str, password: &str) -> Result<(), String> {
    keyring::Entry::new(VNC_KEYRING_SERVICE, rental_id)
        .map_err(|e| e.to_string())?
        .set_password(password)
        .map_err(|e| e.to_string())
}

pub fn load_vnc_password(rental_id: &str) -> Option<String> {
    keyring::Entry::new(VNC_KEYRING_SERVICE, rental_id)
        .ok()?
        .get_password()
        .ok()
}

pub fn delete_vnc_password(rental_id: &str) {
    if let Ok(entry) = keyring::Entry::new(VNC_KEYRING_SERVICE, rental_id) {
        let _ = entry.delete_credential();
    }
}

pub fn delete_stored_key(rental_id: &str) {
    if let Ok(entry) = keyring::Entry::new(SSH_KEYRING_SERVICE, rental_id) {
        let _ = entry.delete_credential();
    }
}

/// Writes the keyring-stored private key out to a `0600` file so it can be
/// handed to `ansible-playbook` as `ansible_ssh_private_key_file` (which
/// needs a real path, not a keyring handle). Callers must delete this file
/// right after use — the keyring copy is what persists for the rental's
/// lifetime, the file copy should never linger on disk.
pub fn materialize_key_file(app: &AppHandle, rental_id: &str) -> Result<PathBuf, String> {
    let pem = load_private_key(rental_id)?;
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?
        .join("ssh");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{rental_id}.pem"));
    std::fs::write(&path, pem).map_err(|e| e.to_string())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }

    Ok(path)
}

/// Deletes a file previously returned by `materialize_key_file`. Best-effort
/// — used in a `defer`-style cleanup after each Ansible/SSH invocation.
pub fn remove_key_file(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
}

/// Accepts any server host key. This app only ever connects to instances it
/// just created itself moments earlier via a freshly imported key pair —
/// there is no prior known-hosts entry and no independent way to verify a
/// host key without SSH access in the first place, so this is a deliberate,
/// scoped trust-on-first-use relaxation (mirrors the `StrictHostKeyChecking
/// no` used for the equivalent Ansible connection), not an oversight.
pub struct AcceptAllHandler;

impl russh::client::Handler for AcceptAllHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

/// Opens an authenticated SSH client session to `host:22` as `user` using
/// the rental's stored keypair. Shared by the readiness probe
/// (`aws::instance::wait_for_ssh_ready`, which connects as `ubuntu` — the
/// only account provisioned before Ansible has run) and the interactive
/// terminal (which connects as the rental's own VM user, once Ansible has
/// authorized this same key for that account too).
pub async fn connect(
    host: &str,
    user: &str,
    private_key_pem: &str,
) -> Result<russh::client::Handle<AcceptAllHandler>, String> {
    let key = decode_secret_key(private_key_pem, None).map_err(|e| e.to_string())?;

    let config = Arc::new(russh::client::Config {
        inactivity_timeout: Some(Duration::from_secs(30)),
        ..Default::default()
    });

    // A blocked network path (security group, missing route) drops packets
    // silently rather than refusing the connection, so the underlying TCP
    // connect can hang on OS-level retries for a minute or more per attempt
    // with no explicit bound here otherwise — starving the caller's retry
    // loop (e.g. `wait_for_ssh_ready`) of the many quick attempts it expects
    // to make within its budget, and making "still booting" indistinguishable
    // from "permanently blocked" in the resulting timeout.
    let mut handle = tokio::time::timeout(
        Duration::from_secs(8),
        russh::client::connect(config, (host, 22), AcceptAllHandler),
    )
    .await
    .map_err(|_| format!("Timed out opening SSH connection to {host}"))?
    .map_err(|e| format!("Could not open SSH connection to {host}: {e}"))?;

    let auth = handle
        .authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), None))
        .await
        .map_err(|e| format!("SSH authentication failed: {e}"))?;

    match auth {
        russh::client::AuthResult::Success => Ok(handle),
        russh::client::AuthResult::Failure { .. } => {
            Err("SSH authentication was rejected".to_string())
        }
    }
}
