use crate::rentals;
use crate::ssh::terminal;
use tauri::AppHandle;

#[tauri::command]
pub async fn open_ssh_terminal(app: AppHandle, rental_id: String) -> Result<String, String> {
    let record = rentals::get_record(&app, &rental_id)
        .await
        .ok_or_else(|| format!("No rental with id {rental_id}"))?;
    let host = record
        .public_ip
        .ok_or_else(|| "Rental has no public IP yet".to_string())?;
    terminal::open(app, rental_id, host, record.vm_username).await
}

#[tauri::command]
pub async fn write_terminal(
    app: AppHandle,
    session_id: String,
    data: String,
) -> Result<(), String> {
    terminal::write(&app, &session_id, data.into_bytes()).await
}

#[tauri::command]
pub async fn resize_terminal(
    app: AppHandle,
    session_id: String,
    cols: u32,
    rows: u32,
) -> Result<(), String> {
    terminal::resize(&app, &session_id, cols, rows).await
}

#[tauri::command]
pub async fn close_terminal(app: AppHandle, session_id: String) -> Result<(), String> {
    terminal::close(&app, &session_id).await
}
