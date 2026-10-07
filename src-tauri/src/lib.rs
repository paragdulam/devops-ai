mod ansible;
mod aws;
mod commands;
mod github;
mod project_kind;
mod rentals;
mod ssh;
mod toolchain;

use commands::accounts::{
    add_cloud_account, delete_cloud_account, list_cloud_accounts, AccountsFileLock,
};
use commands::github::{
    delete_github_account, detect_github_project, get_github_link_status, link_github_account,
    list_github_accounts, list_github_repos,
};
use commands::rentals::{
    get_provisioning_log, get_rental, get_rental_actual_cost, list_rentals, refresh_rental_access,
    start_rental, stop_rental,
};
use commands::terminal::{close_terminal, open_ssh_terminal, resize_terminal, write_terminal};
use github::GithubLinkState;
use rentals::{ActualCostCache, RentalsState};
use ssh::terminal::SshTerminalState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AccountsFileLock::default())
        .manage(RentalsState::default())
        .manage(ActualCostCache::default())
        .manage(GithubLinkState::default())
        .manage(SshTerminalState::default())
        .setup(|app| {
            // Re-attach to rentals that outlived the previous run.
            let handle = app.handle().clone();
            for id in rentals::restore(&handle) {
                tauri::async_runtime::spawn(rentals::reconcile(handle.clone(), id));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_cloud_accounts,
            add_cloud_account,
            delete_cloud_account,
            start_rental,
            get_rental,
            list_rentals,
            get_rental_actual_cost,
            refresh_rental_access,
            get_provisioning_log,
            stop_rental,
            link_github_account,
            get_github_link_status,
            list_github_accounts,
            delete_github_account,
            list_github_repos,
            detect_github_project,
            open_ssh_terminal,
            write_terminal,
            resize_terminal,
            close_terminal,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
