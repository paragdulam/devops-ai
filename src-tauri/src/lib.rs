mod ansible;
mod aws;
mod commands;
mod github;
mod project_kind;
mod rentals;
mod ssh;

use commands::accounts::{add_cloud_account, delete_cloud_account, list_cloud_accounts, AccountsFileLock};
use commands::github::{
    delete_github_account, detect_github_project_kind, get_github_link_status, link_github_account, list_github_accounts,
    list_github_repos,
};
use commands::project::inspect_project_folder;
use commands::rentals::{get_provisioning_log, get_rental, start_rental, stop_rental};
use commands::terminal::{close_terminal, open_ssh_terminal, resize_terminal, write_terminal};
use github::GithubLinkState;
use rentals::RentalsState;
use ssh::terminal::SshTerminalState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AccountsFileLock::default())
        .manage(RentalsState::default())
        .manage(GithubLinkState::default())
        .manage(SshTerminalState::default())
        .invoke_handler(tauri::generate_handler![
            inspect_project_folder,
            list_cloud_accounts,
            add_cloud_account,
            delete_cloud_account,
            start_rental,
            get_rental,
            get_provisioning_log,
            stop_rental,
            link_github_account,
            get_github_link_status,
            list_github_accounts,
            delete_github_account,
            list_github_repos,
            detect_github_project_kind,
            open_ssh_terminal,
            write_terminal,
            resize_terminal,
            close_terminal,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
