use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tokio::sync::Mutex;

const KEYRING_SERVICE: &str = "com.paragdulam.remotedevmachine.accounts";
const ACCOUNTS_FILE: &str = "accounts.json";

#[derive(Default)]
pub struct AccountsFileLock(pub Mutex<()>);

// `provider` is a free string today (only "aws" is accepted) so a second
// cloud provider can be added later without restructuring this type.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudAccount {
    pub id: String,
    pub label: String,
    pub provider: String,
    pub region: String,
    pub access_key_id: String,
    pub created_at: String,
}

fn accounts_file_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(ACCOUNTS_FILE))
}

fn read_accounts(app: &AppHandle) -> Result<Vec<CloudAccount>, String> {
    let path = accounts_file_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn write_accounts(app: &AppHandle, accounts: &[CloudAccount]) -> Result<(), String> {
    let path = accounts_file_path(app)?;
    let raw = serde_json::to_string_pretty(accounts).map_err(|e| e.to_string())?;
    std::fs::write(&path, raw).map_err(|e| e.to_string())
}

async fn validate_aws_credentials(
    region: &str,
    access_key_id: &str,
    secret_access_key: &str,
) -> Result<(), String> {
    let config = crate::aws::credentials::build_sdk_config(region, access_key_id, secret_access_key).await;
    let sts = aws_sdk_sts::Client::new(&config);
    sts.get_caller_identity()
        .send()
        .await
        .map_err(|e| format!("Could not validate AWS credentials: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn list_cloud_accounts(app: AppHandle) -> Result<Vec<CloudAccount>, String> {
    read_accounts(&app)
}

#[tauri::command]
pub async fn add_cloud_account(
    app: AppHandle,
    lock: State<'_, AccountsFileLock>,
    label: String,
    provider: String,
    region: String,
    access_key_id: String,
    secret_access_key: String,
) -> Result<CloudAccount, String> {
    if provider != "aws" {
        return Err(format!("Unsupported provider: {provider}"));
    }

    validate_aws_credentials(&region, &access_key_id, &secret_access_key).await?;

    let id = uuid::Uuid::new_v4().to_string();
    keyring::Entry::new(KEYRING_SERVICE, &id)
        .map_err(|e| e.to_string())?
        .set_password(&secret_access_key)
        .map_err(|e| e.to_string())?;

    let account = CloudAccount {
        id,
        label,
        provider,
        region,
        access_key_id,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let _guard = lock.0.lock().await;
    let mut accounts = read_accounts(&app)?;
    accounts.push(account.clone());
    write_accounts(&app, &accounts)?;

    Ok(account)
}

#[tauri::command]
pub async fn delete_cloud_account(
    app: AppHandle,
    lock: State<'_, AccountsFileLock>,
    id: String,
) -> Result<(), String> {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, &id) {
        let _ = entry.delete_credential();
    }

    let _guard = lock.0.lock().await;
    let mut accounts = read_accounts(&app)?;
    accounts.retain(|a| a.id != id);
    write_accounts(&app, &accounts)
}

/// Not a Tauri command — used internally by the provisioning code to fetch an
/// account plus its secret together. Never exposed over IPC.
pub fn resolve_account_secret(
    app: &AppHandle,
    id: &str,
) -> Result<(CloudAccount, String), String> {
    let accounts = read_accounts(app)?;
    let account = accounts
        .into_iter()
        .find(|a| a.id == id)
        .ok_or_else(|| format!("No saved account with id {id}"))?;
    let secret = keyring::Entry::new(KEYRING_SERVICE, &account.id)
        .map_err(|e| e.to_string())?
        .get_password()
        .map_err(|e| e.to_string())?;
    Ok((account, secret))
}
