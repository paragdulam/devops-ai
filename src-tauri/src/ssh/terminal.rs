use crate::ssh;
use russh::client::Msg;
use russh::{ChannelMsg, ChannelWriteHalf};
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;

pub struct SshTerminalSession {
    write_half: ChannelWriteHalf<Msg>,
}

#[derive(Default)]
pub struct SshTerminalState(pub Mutex<HashMap<String, SshTerminalSession>>);

fn state(app: &AppHandle) -> tauri::State<'_, SshTerminalState> {
    app.state::<SshTerminalState>()
}

/// Opens an interactive SSH PTY session to `host` as `ubuntu`, using the
/// rental's stored keypair (the same one Ansible provisioned with). Returns
/// a session id the frontend uses for subsequent writes/resizes/close and to
/// filter the `ssh-terminal://{session_id}/data` event stream.
pub async fn open(
    app: AppHandle,
    rental_id: String,
    host: String,
    user: String,
) -> Result<String, String> {
    let private_key_pem = ssh::load_private_key(&rental_id)?;
    let handle = ssh::connect(&host, &user, &private_key_pem).await?;

    let channel = handle
        .channel_open_session()
        .await
        .map_err(|e| format!("Could not open SSH channel: {e}"))?;
    channel
        .request_pty(true, "xterm-256color", 80, 24, 0, 0, &[])
        .await
        .map_err(|e| format!("Could not request a PTY: {e}"))?;
    channel
        .request_shell(true)
        .await
        .map_err(|e| format!("Could not start a shell: {e}"))?;

    let (mut read_half, write_half) = channel.split();
    let session_id = uuid::Uuid::new_v4().to_string();

    // Read pump: owns the connection handle and the read half for the whole
    // session, emitting one event per chunk of output. Runs independently of
    // the state lock — never holds it across this loop, only to
    // register/deregister the session (mirrors GithubLinkState's background
    // poll task in github/mod.rs).
    let app_for_task = app.clone();
    let session_id_for_task = session_id.clone();
    tauri::async_runtime::spawn(async move {
        let _handle = handle; // keeps the SSH connection alive for the task's lifetime
        loop {
            match read_half.wait().await {
                Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                    let _ = app_for_task.emit(
                        &format!("ssh-terminal://{session_id_for_task}/data"),
                        String::from_utf8_lossy(&data).to_string(),
                    );
                }
                Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                _ => {}
            }
        }
        state(&app_for_task)
            .0
            .lock()
            .await
            .remove(&session_id_for_task);
    });

    {
        let s = state(&app);
        let mut map = s.0.lock().await;
        map.insert(session_id.clone(), SshTerminalSession { write_half });
    }

    Ok(session_id)
}

pub async fn write(app: &AppHandle, session_id: &str, data: Vec<u8>) -> Result<(), String> {
    let s = state(app);
    let map = s.0.lock().await;
    let session = map
        .get(session_id)
        .ok_or_else(|| format!("No terminal session {session_id}"))?;
    session
        .write_half
        .data_bytes(data)
        .await
        .map_err(|e| e.to_string())
}

pub async fn resize(app: &AppHandle, session_id: &str, cols: u32, rows: u32) -> Result<(), String> {
    let s = state(app);
    let map = s.0.lock().await;
    let session = map
        .get(session_id)
        .ok_or_else(|| format!("No terminal session {session_id}"))?;
    session
        .write_half
        .window_change(cols, rows, 0, 0)
        .await
        .map_err(|e| e.to_string())
}

pub async fn close(app: &AppHandle, session_id: &str) -> Result<(), String> {
    let s = state(app);
    let mut map = s.0.lock().await;
    if let Some(session) = map.remove(session_id) {
        let _ = session.write_half.close().await;
    }
    Ok(())
}
