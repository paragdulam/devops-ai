use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

const KEYRING_SERVICE: &str = "com.paragdulam.remotedevmachine.github";
const ACCOUNTS_FILE: &str = "github_accounts.json";

// GitHub OAuth App "Remote Dev Machine (Local)" (github.com/settings/applications/3869293),
// registered under paragdulam with Device Flow enabled. Public identifier, not a secret —
// device flow needs no client secret.
const GITHUB_CLIENT_ID: &str = "Ov23liJkAy6YW8ZCZwQN";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubAccount {
    pub id: String,
    pub login: String,
    pub avatar_url: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubRepo {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub private: bool,
    pub clone_url: String,
    pub default_branch: String,
}

#[derive(Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum LinkStatus {
    Pending,
    Linked { account: GithubAccount },
    Failed { error: String },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkStart {
    pub link_id: String,
    pub user_code: String,
    pub verification_uri: String,
}

#[derive(Default)]
pub struct GithubLinkState(pub Mutex<HashMap<String, LinkStatus>>);

fn link_state(app: &AppHandle) -> tauri::State<'_, GithubLinkState> {
    app.state::<GithubLinkState>()
}

fn accounts_file_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(ACCOUNTS_FILE))
}

fn read_accounts(app: &AppHandle) -> Result<Vec<GithubAccount>, String> {
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

fn write_accounts(app: &AppHandle, accounts: &[GithubAccount]) -> Result<(), String> {
    let path = accounts_file_path(app)?;
    let raw = serde_json::to_string_pretty(accounts).map_err(|e| e.to_string())?;
    std::fs::write(&path, raw).map_err(|e| e.to_string())
}

pub fn list_accounts(app: &AppHandle) -> Result<Vec<GithubAccount>, String> {
    read_accounts(app)
}

pub fn delete_account(app: &AppHandle, id: &str) -> Result<(), String> {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, id) {
        let _ = entry.delete_credential();
    }
    let mut accounts = read_accounts(app)?;
    accounts.retain(|a| a.id != id);
    write_accounts(app, &accounts)
}

/// Refresh this long before the access token's expiry, so a token handed out
/// here stays valid for the API calls (or `git clone`) that follow.
const REFRESH_MARGIN_SECS: i64 = 10 * 60;

/// What's kept in the keyring per account. GitHub issues user access tokens
/// that expire after 8 hours, plus a refresh token (valid 6 months) that
/// trades for a fresh pair — so the link lasts as long as the app is used at
/// least every 6 months. A plain token string (no JSON) is the pre-refresh
/// format; it still works until GitHub expires it, then needs a re-link.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct StoredToken {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    /// Unix seconds; `None` for tokens that don't expire.
    #[serde(default)]
    expires_at: Option<i64>,
}

impl StoredToken {
    fn parse(raw: &str) -> StoredToken {
        serde_json::from_str(raw).unwrap_or_else(|_| StoredToken {
            access_token: raw.to_string(),
            refresh_token: None,
            expires_at: None,
        })
    }

    fn from_grant(grant: TokenGrant, now: i64) -> StoredToken {
        StoredToken {
            access_token: grant.access_token,
            refresh_token: grant.refresh_token,
            expires_at: grant.expires_in.map(|s| now + s as i64),
        }
    }

    fn needs_refresh(&self, now: i64) -> bool {
        self.expires_at
            .is_some_and(|at| now >= at - REFRESH_MARGIN_SECS)
    }
}

/// Serializes refreshes: GitHub rotates the refresh token on use, so two
/// concurrent refreshes of one account would leave the loser with a dead one.
static REFRESH_LOCK: Mutex<()> = Mutex::const_new(());

fn keyring_entry(id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, id).map_err(|e| e.to_string())
}

fn save_token(id: &str, token: &StoredToken) -> Result<(), String> {
    let raw = serde_json::to_string(token).map_err(|e| e.to_string())?;
    keyring_entry(id)?
        .set_password(&raw)
        .map_err(|e| e.to_string())
}

fn load_token(id: &str) -> Result<StoredToken, String> {
    keyring_entry(id)?
        .get_password()
        .map(|raw| StoredToken::parse(&raw))
        .map_err(|e| e.to_string())
}

async fn refresh_token(client: &reqwest::Client, refresh_token: &str) -> Result<TokenGrant, String> {
    // Tokens from the device flow refresh without a client secret.
    let resp: AccessTokenResponse = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", GITHUB_CLIENT_ID),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .map_err(|e| format!("Could not refresh GitHub token: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Could not parse GitHub token refresh response: {e}"))?;
    match resp {
        AccessTokenResponse::Success(grant) => Ok(grant),
        AccessTokenResponse::Error { error } => Err(format!(
            "GitHub sign-in expired ({error}). Unlink the GitHub account and link it again."
        )),
    }
}

/// Not a Tauri command — used internally by `list_github_repos` and by rental
/// provisioning to fetch a saved account's OAuth token, refreshing it first
/// when it's about to expire. Never exposed over IPC.
pub async fn resolve_github_token(id: &str) -> Result<String, String> {
    let now = chrono::Utc::now().timestamp();
    let token = load_token(id)?;
    if !token.needs_refresh(now) {
        return Ok(token.access_token);
    }

    let _guard = REFRESH_LOCK.lock().await;
    // Another caller may have refreshed while this one waited for the lock.
    let token = load_token(id)?;
    if !token.needs_refresh(now) {
        return Ok(token.access_token);
    }
    let Some(refresh) = token.refresh_token.as_deref() else {
        return Err(
            "GitHub sign-in expired. Unlink the GitHub account and link it again.".to_string(),
        );
    };
    let grant = refresh_token(&reqwest::Client::new(), refresh).await?;
    let fresh = StoredToken::from_grant(grant, chrono::Utc::now().timestamp());
    save_token(id, &fresh)?;
    Ok(fresh.access_token)
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Debug, Deserialize)]
struct TokenGrant {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    /// Seconds until `access_token` expires; absent for non-expiring tokens.
    #[serde(default)]
    expires_in: Option<u64>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum AccessTokenResponse {
    Success(TokenGrant),
    Error { error: String },
}

async fn request_device_code(client: &reqwest::Client) -> Result<DeviceCodeResponse, String> {
    client
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[("client_id", GITHUB_CLIENT_ID), ("scope", "repo")])
        .send()
        .await
        .map_err(|e| format!("Could not start GitHub device flow: {e}"))?
        .json::<DeviceCodeResponse>()
        .await
        .map_err(|e| format!("Could not parse GitHub device flow response: {e}"))
}

async fn poll_for_token(
    client: &reqwest::Client,
    device_code: &str,
    interval: u64,
    expires_in: u64,
) -> Result<TokenGrant, String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(expires_in);
    let mut wait = Duration::from_secs(interval.max(1));

    loop {
        tokio::time::sleep(wait).await;
        if tokio::time::Instant::now() >= deadline {
            return Err("Device code expired before it was approved".to_string());
        }

        let resp: AccessTokenResponse = client
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .form(&[
                ("client_id", GITHUB_CLIENT_ID),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .map_err(|e| format!("Could not poll for GitHub token: {e}"))?
            .json()
            .await
            .map_err(|e| format!("Could not parse GitHub token response: {e}"))?;

        match resp {
            AccessTokenResponse::Success(grant) => return Ok(grant),
            AccessTokenResponse::Error { error } => match error.as_str() {
                "authorization_pending" => continue,
                "slow_down" => {
                    wait += Duration::from_secs(5);
                    continue;
                }
                "expired_token" => {
                    return Err("Device code expired before it was approved".to_string())
                }
                "access_denied" => return Err("GitHub authorization was denied".to_string()),
                other => return Err(format!("GitHub device flow error: {other}")),
            },
        }
    }
}

#[derive(Deserialize)]
struct GithubUser {
    login: String,
    avatar_url: Option<String>,
}

async fn fetch_viewer(client: &reqwest::Client, token: &str) -> Result<GithubUser, String> {
    client
        .get("https://api.github.com/user")
        .bearer_auth(token)
        .header("User-Agent", "remote-dev-machine")
        .send()
        .await
        .map_err(|e| format!("Could not fetch GitHub user: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Could not parse GitHub user response: {e}"))
}

async fn finish_link(app: &AppHandle, grant: TokenGrant, client: &reqwest::Client) -> LinkStatus {
    let token = StoredToken::from_grant(grant, chrono::Utc::now().timestamp());
    let user = match fetch_viewer(client, &token.access_token).await {
        Ok(u) => u,
        Err(e) => return LinkStatus::Failed { error: e },
    };

    let id = uuid::Uuid::new_v4().to_string();
    if let Err(error) = save_token(&id, &token) {
        return LinkStatus::Failed { error };
    }

    let account = GithubAccount {
        id,
        login: user.login,
        avatar_url: user.avatar_url,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let save_result: Result<(), String> = (|| {
        let mut accounts = read_accounts(app)?;
        accounts.push(account.clone());
        write_accounts(app, &accounts)
    })();

    match save_result {
        Ok(()) => LinkStatus::Linked { account },
        Err(e) => LinkStatus::Failed { error: e },
    }
}

/// Kicks off the device flow and returns immediately with the code the user
/// needs to approve in their browser. The actual wait-for-approval happens in
/// a spawned background task; poll `get_github_link_status` for the outcome.
pub async fn start_link(app: AppHandle) -> Result<LinkStart, String> {
    let client = reqwest::Client::new();
    let device = request_device_code(&client).await?;
    let link_id = uuid::Uuid::new_v4().to_string();

    {
        let state = link_state(&app);
        let mut map = state.0.lock().await;
        map.insert(link_id.clone(), LinkStatus::Pending);
    }

    let response = LinkStart {
        link_id: link_id.clone(),
        user_code: device.user_code.clone(),
        verification_uri: device.verification_uri.clone(),
    };

    let app_for_task = app.clone();
    tauri::async_runtime::spawn(async move {
        let client = reqwest::Client::new();
        let status = match poll_for_token(
            &client,
            &device.device_code,
            device.interval,
            device.expires_in,
        )
        .await
        {
            Ok(grant) => finish_link(&app_for_task, grant, &client).await,
            Err(e) => LinkStatus::Failed { error: e },
        };
        let state = link_state(&app_for_task);
        let mut map = state.0.lock().await;
        map.insert(link_id, status);
    });

    Ok(response)
}

#[derive(Deserialize)]
struct RawRepo {
    id: i64,
    name: String,
    full_name: String,
    private: bool,
    clone_url: String,
    default_branch: String,
}

/// Lists the linked account's repos (owned + collaborator + org member),
/// most-recently-updated first. Paginates up to a few pages — enough for MVP
/// browsing without pulling in every repo a heavy GitHub user has ever touched.
pub async fn list_repos(token: &str) -> Result<Vec<GithubRepo>, String> {
    let client = reqwest::Client::new();
    let mut repos = Vec::new();

    for page in 1..=3u32 {
        let page_str = page.to_string();
        let raw: Vec<RawRepo> = client
            .get("https://api.github.com/user/repos")
            .bearer_auth(token)
            .header("User-Agent", "remote-dev-machine")
            .query(&[
                ("per_page", "100"),
                ("sort", "updated"),
                ("affiliation", "owner,collaborator,organization_member"),
                ("page", page_str.as_str()),
            ])
            .send()
            .await
            .map_err(|e| format!("Could not list GitHub repos: {e}"))?
            .json()
            .await
            .map_err(|e| format!("Could not parse GitHub repos response: {e}"))?;

        let got = raw.len();
        repos.extend(raw.into_iter().map(|r| GithubRepo {
            id: r.id,
            name: r.name,
            full_name: r.full_name,
            private: r.private,
            clone_url: r.clone_url,
            default_branch: r.default_branch,
        }));

        if got < 100 {
            break;
        }
    }

    Ok(repos)
}

/// Fetches whichever of `paths` exist at the tip of `branch`, via the
/// contents API (raw media type), concurrently. A missing file (404) or any
/// per-file failure just leaves it out — detection treats absent files the
/// same either way.
pub async fn fetch_repo_files(
    token: &str,
    full_name: &str,
    branch: &str,
    paths: impl IntoIterator<Item = &'static str>,
) -> std::collections::HashMap<&'static str, String> {
    let client = reqwest::Client::new();
    let mut tasks = tokio::task::JoinSet::new();

    for path in paths {
        let request = client
            .get(format!(
                "https://api.github.com/repos/{full_name}/contents/{path}"
            ))
            .bearer_auth(token)
            .header("User-Agent", "remote-dev-machine")
            .header("Accept", "application/vnd.github.raw+json")
            .query(&[("ref", branch)]);
        tasks.spawn(async move {
            let response = request.send().await.ok()?.error_for_status().ok()?;
            Some((path, response.text().await.ok()?))
        });
    }

    let mut files = std::collections::HashMap::new();
    while let Some(result) = tasks.join_next().await {
        if let Ok(Some((path, content))) = result {
            files.insert(path, content);
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_plain_token_parses_without_expiry() {
        let t = StoredToken::parse("gho_plain");
        assert_eq!(t.access_token, "gho_plain");
        assert_eq!(t.refresh_token, None);
        assert!(!t.needs_refresh(i64::MAX));
    }

    #[test]
    fn stored_token_round_trips_through_json() {
        let t = StoredToken {
            access_token: "ghu_a".into(),
            refresh_token: Some("ghr_r".into()),
            expires_at: Some(1_000),
        };
        assert_eq!(StoredToken::parse(&serde_json::to_string(&t).unwrap()), t);
    }

    #[test]
    fn refreshes_shortly_before_expiry() {
        let grant: TokenGrant = serde_json::from_str(
            r#"{"access_token":"ghu_a","expires_in":28800,"refresh_token":"ghr_r",
                "refresh_token_expires_in":15811200,"token_type":"bearer","scope":"repo"}"#,
        )
        .unwrap();
        let t = StoredToken::from_grant(grant, 0);
        assert_eq!(t.expires_at, Some(28800));
        assert!(!t.needs_refresh(28800 - REFRESH_MARGIN_SECS - 1));
        assert!(t.needs_refresh(28800 - REFRESH_MARGIN_SECS));
    }

    #[test]
    fn token_response_variants_parse() {
        let ok: AccessTokenResponse =
            serde_json::from_str(r#"{"access_token":"gho_x","token_type":"bearer","scope":"repo"}"#)
                .unwrap();
        assert!(matches!(ok, AccessTokenResponse::Success(g) if g.expires_in.is_none()));
        let err: AccessTokenResponse =
            serde_json::from_str(r#"{"error":"authorization_pending"}"#).unwrap();
        assert!(matches!(err, AccessTokenResponse::Error { .. }));
    }
}
