use crate::github::{self, GithubAccount, GithubLinkState, GithubRepo, LinkStart, LinkStatus};
use crate::project_kind::{self, ProjectKind};
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn link_github_account(app: AppHandle) -> Result<LinkStart, String> {
    github::start_link(app).await
}

#[tauri::command]
pub async fn get_github_link_status(
    state: State<'_, GithubLinkState>,
    link_id: String,
) -> Result<LinkStatus, String> {
    let map = state.0.lock().await;
    map.get(&link_id)
        .cloned()
        .ok_or_else(|| format!("No such GitHub link session: {link_id}"))
}

#[tauri::command]
pub async fn list_github_accounts(app: AppHandle) -> Result<Vec<GithubAccount>, String> {
    github::list_accounts(&app)
}

#[tauri::command]
pub async fn delete_github_account(app: AppHandle, id: String) -> Result<(), String> {
    github::delete_account(&app, &id)
}

#[tauri::command]
pub async fn list_github_repos(account_id: String) -> Result<Vec<GithubRepo>, String> {
    let token = github::resolve_github_token(&account_id)?;
    github::list_repos(&token).await
}

#[tauri::command]
pub async fn detect_github_project_kind(
    account_id: String,
    full_name: String,
    branch: String,
) -> Result<ProjectKind, String> {
    let token = github::resolve_github_token(&account_id)?;
    let files = github::fetch_marker_files(&token, &full_name, &branch).await;
    Ok(project_kind::classify(&files))
}
