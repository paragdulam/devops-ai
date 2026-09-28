use serde::Deserialize;
use sha_crypt::{PasswordHasher, ShaCrypt};
use std::path::PathBuf;
use std::process::Stdio;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// A GitHub repo to clone onto the instance, resolved the same way the old
/// cloud-init `CloneSpec` was — see `rentals::provision`.
pub struct CloneSpec {
    pub url: String,
    pub branch: String,
    pub dir_name: String,
}

pub struct ProvisionParams<'a> {
    pub app: &'a AppHandle,
    pub rental_id: &'a str,
    pub host: &'a str,
    pub private_key_path: &'a std::path::Path,
    pub vm_username: &'a str,
    pub vm_password: &'a str,
    /// The same OpenSSH public-key line imported as the EC2 keypair —
    /// authorized for the new VM user too, so the on-demand terminal can
    /// connect as that user (not just the bootstrap `ubuntu` account
    /// Ansible itself uses).
    pub authorized_key_line: &'a str,
    /// The remote-desktop (VNC) session password — separate from the OS
    /// login password, generated the same way it always was
    /// (`ssh::generate_vnc_password`), just fed to Ansible instead of
    /// cloud-init now.
    pub vnc_password: &'a str,
    pub profile_packages: &'a [&'static str],
    pub clone: Option<&'a CloneSpec>,
}

const SALT_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789./";

/// Hashes the VM login password in Rust (SHA-512-crypt, the format `/etc/
/// shadow` and Ansible's `user` module both expect) rather than via
/// Ansible's Jinja `password_hash` filter, which needs `passlib` installed
/// in the Python environment `ansible-playbook` runs under — this keeps
/// `ansible-core` the only external requirement. The plaintext password is
/// never templated into the generated playbook.
fn hash_password(password: &str) -> Result<String, String> {
    let salt: String = (0..16)
        .map(|_| SALT_CHARS[rand::random_range(0..SALT_CHARS.len())] as char)
        .collect();
    let hash = ShaCrypt::SHA512
        .hash_password_with_salt(password.as_bytes(), salt.as_bytes())
        .map_err(|e| format!("Could not hash VM password: {e:?}"))?;
    Ok(hash.as_str().to_string())
}

/// Fails fast, before any AWS spend, if Ansible isn't on PATH.
pub async fn preflight() -> Result<(), String> {
    let status = Command::new("ansible-playbook")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|_| {
            "ansible-playbook not found on PATH — install Ansible (e.g. `brew install ansible` \
             or `pipx install ansible-core`) before renting a machine"
                .to_string()
        })?;
    if !status.success() {
        return Err(
            "`ansible-playbook --version` exited with an error — check your Ansible installation"
                .to_string(),
        );
    }
    Ok(())
}

fn artifact_dir(app: &AppHandle, rental_id: &str) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?
        .join("rentals")
        .join(rental_id)
        .join("ansible");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Deletes the whole per-rental artifact directory (inventory + playbook) —
/// these shouldn't outlive the rental any more than the materialized SSH key
/// file does. Best-effort, called from `rentals::cleanup`.
pub fn remove_artifacts(app: &AppHandle, rental_id: &str) {
    if let Ok(dir) = app.path().app_local_data_dir() {
        let _ = std::fs::remove_dir_all(dir.join("rentals").join(rental_id));
    }
}

fn build_inventory(host: &str, key_path: &std::path::Path) -> String {
    // The key path is unquoted, so it must not contain a space — but
    // `app_local_data_dir()` on macOS resolves under `~/Library/Application
    // Support/...`, which always does. An unquoted value there breaks the
    // ini inventory parser, which then silently falls back to "only
    // implicit localhost available" — the play (`hosts: vm`) matches zero
    // hosts, every task is skipped, and `ansible-playbook` still exits 0,
    // making the whole provisioning step look like a no-op success.
    format!(
        "[vm]\n{host} ansible_user=ubuntu ansible_ssh_private_key_file=\"{key}\" ansible_ssh_common_args='-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null'\n",
        host = host,
        key = key_path.display(),
    )
}

/// Builds the playbook as a formatted string — same technique the old
/// `aws::userdata::build_user_data` used for `#cloud-config`, new target
/// format. Task order matches the "desktop setup before clone" requirement:
/// base packages -> create user -> desktop/VNC/websockify (systemd-managed)
/// -> profile packages -> repo clone.
fn build_playbook(
    vm_username: &str,
    password_hash: &str,
    authorized_key_line: &str,
    vnc_password: &str,
    profile_packages: &[&str],
    clone: Option<&CloneSpec>,
) -> String {
    // `name: []` must stay on one line for an empty list — YAML doesn't allow
    // a bare `[]` (no list-dash) on its own line as a mapping value's
    // continuation, only actual `- item` block-sequence entries can follow
    // the key on the next line.
    let profile_packages_yaml = if profile_packages.is_empty() {
        "        name: []".to_string()
    } else {
        let items = profile_packages
            .iter()
            .map(|p| format!("        - {p}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("        name:\n{items}")
    };

    let clone_task = clone
        .map(|c| {
            format!(
                r#"
    - name: Clone the selected repository
      tags: [clone]
      become_user: "{vm_username}"
      environment:
        GIT_TERMINAL_PROMPT: "0"
      ansible.builtin.git:
        repo: "{url}"
        version: "{branch}"
        dest: "/home/{vm_username}/Documents/Projects/{dir}"
        depth: 1
"#,
                vm_username = vm_username,
                url = c.url,
                branch = c.branch,
                dir = c.dir_name,
            )
        })
        .unwrap_or_default();

    format!(
        r#"---
- name: Provision Remote Dev Machine
  hosts: vm
  become: true
  gather_facts: true

  tasks:
    - name: Update apt cache and install base packages
      tags: [base]
      ansible.builtin.apt:
        update_cache: true
        name:
          - xfce4
          - xfce4-goodies
          - dbus-x11
          - tigervnc-standalone-server
          - novnc
          - python3-websockify
          - git

    - name: Create the VM login user
      tags: [user]
      ansible.builtin.user:
        name: "{vm_username}"
        groups: sudo
        append: true
        shell: /bin/bash
        password: "{password_hash}"
        create_home: true

    - name: Create ~/.ssh for the VM user
      tags: [user]
      ansible.builtin.file:
        path: "/home/{vm_username}/.ssh"
        state: directory
        owner: "{vm_username}"
        group: "{vm_username}"
        mode: "0700"

    - name: Authorize the rental's SSH key for the VM user
      tags: [user]
      ansible.builtin.copy:
        dest: "/home/{vm_username}/.ssh/authorized_keys"
        owner: "{vm_username}"
        group: "{vm_username}"
        mode: "0600"
        content: |
          {authorized_key_line}

    - name: Create ~/.vnc for the VM user
      tags: [desktop]
      ansible.builtin.file:
        path: "/home/{vm_username}/.vnc"
        state: directory
        owner: "{vm_username}"
        group: "{vm_username}"
        mode: "0700"

    - name: Write the VNC password file
      tags: [desktop]
      ansible.builtin.shell: |
        echo "{vnc_password}" | vncpasswd -f > /home/{vm_username}/.vnc/passwd
      args:
        creates: "/home/{vm_username}/.vnc/passwd"

    - name: Fix ownership/permissions on the VNC password file
      tags: [desktop]
      ansible.builtin.file:
        path: "/home/{vm_username}/.vnc/passwd"
        owner: "{vm_username}"
        group: "{vm_username}"
        mode: "0600"

    - name: Write the VNC xstartup script
      tags: [desktop]
      ansible.builtin.copy:
        dest: "/home/{vm_username}/.vnc/xstartup"
        owner: "{vm_username}"
        group: "{vm_username}"
        mode: "0755"
        content: |
          #!/bin/sh
          exec startxfce4

    - name: Install the systemd unit for the VNC session
      tags: [desktop]
      ansible.builtin.copy:
        dest: /etc/systemd/system/rdm-vnc.service
        mode: "0644"
        content: |
          [Unit]
          Description=Remote Dev Machine VNC session
          After=network.target

          [Service]
          Type=forking
          User={vm_username}
          WorkingDirectory=/home/{vm_username}
          ExecStart=/usr/bin/vncserver :1 -geometry 1600x900 -depth 24 -localhost yes
          ExecStop=/usr/bin/vncserver -kill :1
          Restart=on-failure

          [Install]
          WantedBy=multi-user.target

    - name: Install the systemd unit for websockify
      tags: [desktop]
      ansible.builtin.copy:
        dest: /etc/systemd/system/rdm-websockify.service
        mode: "0644"
        content: |
          [Unit]
          Description=Remote Dev Machine noVNC websockify bridge
          After=rdm-vnc.service
          Requires=rdm-vnc.service

          [Service]
          Type=simple
          ExecStart=/usr/bin/websockify 0.0.0.0:6080 localhost:5901 --web=/usr/share/novnc/
          Restart=on-failure

          [Install]
          WantedBy=multi-user.target

    - name: Enable and start the VNC and websockify services
      tags: [desktop]
      ansible.builtin.systemd:
        name: "{{{{ item }}}}"
        daemon_reload: true
        enabled: true
        state: started
      loop:
        - rdm-vnc.service
        - rdm-websockify.service

    - name: Install profile-specific packages
      tags: [packages]
      ansible.builtin.apt:
{profile_packages_yaml}
        state: present
{clone_task}"#,
        vm_username = vm_username,
        password_hash = password_hash,
        authorized_key_line = authorized_key_line,
        vnc_password = vnc_password,
        profile_packages_yaml = profile_packages_yaml,
        clone_task = clone_task,
    )
}

#[derive(Deserialize)]
struct NdjsonEvent {
    #[serde(default)]
    event: String,
    #[serde(default)]
    task: String,
    #[serde(default)]
    tags: Vec<String>,
}

/// Maps a playbook task's tags to the coarse phase the UI checklist tracks.
fn phase_for_tags(tags: &[String]) -> Option<&'static str> {
    for tag in tags {
        match tag.as_str() {
            "user" => return Some("user"),
            "desktop" => return Some("desktop"),
            "packages" => return Some("packages"),
            "clone" => return Some("clone"),
            _ => {}
        }
    }
    None
}

/// Generates the inventory + playbook, then runs `ansible-playbook`,
/// streaming each NDJSON line from the bundled callback plugin as a
/// `rental://{id}/log` event, and each recognized task-lifecycle event as a
/// `rental://{id}/step` event. `StrictHostKeyChecking=no` (set on the
/// inventory line) is a deliberate, scoped trust relaxation — this is a
/// freshly created, single-purpose instance with no prior host key to check.
pub async fn provision(params: ProvisionParams<'_>) -> Result<(), String> {
    let dir = artifact_dir(params.app, params.rental_id)?;
    let inventory_path = dir.join("inventory.ini");
    let playbook_path = dir.join("playbook.yml");

    std::fs::write(
        &inventory_path,
        build_inventory(params.host, params.private_key_path),
    )
    .map_err(|e| e.to_string())?;

    let password_hash = hash_password(params.vm_password)?;
    let playbook = build_playbook(
        params.vm_username,
        &password_hash,
        params.authorized_key_line,
        params.vnc_password,
        params.profile_packages,
        params.clone,
    );
    std::fs::write(&playbook_path, &playbook).map_err(|e| e.to_string())?;

    let log_topic = format!("rental://{}/log", params.rental_id);
    let step_topic = format!("rental://{}/step", params.rental_id);

    for line in std::iter::once(format!(
        "--- ansible playbook for rental {} ({}) ---",
        params.rental_id,
        playbook_path.display(),
    ))
    .chain(playbook.lines().map(str::to_string))
    .chain(std::iter::once("--- end playbook ---".to_string()))
    {
        #[cfg(debug_assertions)]
        eprintln!("{line}");

        let _ = params.app.emit(&log_topic, &line);
        crate::rentals::append_log_line(params.app, params.rental_id, line).await;
    }

    let callback_dir = params
        .app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join("ansible")
        .join("callback_plugins");

    let mut child = Command::new("ansible-playbook")
        .arg("-i")
        .arg(&inventory_path)
        .arg(&playbook_path)
        .env("ANSIBLE_HOST_KEY_CHECKING", "False")
        .env("ANSIBLE_STDOUT_CALLBACK", "rdm_ndjson")
        .env("ANSIBLE_CALLBACK_PLUGINS", &callback_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start ansible-playbook: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or("ansible-playbook had no stdout")?;
    let mut lines = BufReader::new(stdout).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        #[cfg(debug_assertions)]
        eprintln!("{line}");

        let _ = params.app.emit(&log_topic, &line);
        crate::rentals::append_log_line(params.app, params.rental_id, line.clone()).await;

        if let Ok(event) = serde_json::from_str::<NdjsonEvent>(&line) {
            if let Some(phase) = phase_for_tags(&event.tags) {
                let _ = params.app.emit(
                    &step_topic,
                    serde_json::json!({
                        "phase": phase,
                        "task": event.task,
                        "state": event.event,
                    }),
                );
            }
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Could not wait on ansible-playbook: {e}"))?;

    if !status.success() {
        return Err(format!(
            "ansible-playbook exited with status {status} — see provisioning logs for details"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::process::Command as StdCommand;

    /// Regression test for a real bug: an empty `profile_packages` list (the
    /// case for every rental today, since `"standard"` is the only machine
    /// profile and it carries none) used to emit a bare `[]` on its own line
    /// under `name:`, which isn't valid YAML for an empty sequence value —
    /// `ansible-playbook` failed with exit code 4 (parser error) on every
    /// single rental. Skips gracefully if `ansible-playbook` isn't on PATH,
    /// so this never becomes a hard CI dependency.
    fn syntax_check(playbook: &str) {
        if StdCommand::new("ansible-playbook")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("skipping: ansible-playbook not on PATH");
            return;
        }

        let mut file = tempfile_path();
        write!(file.1, "{playbook}").unwrap();
        drop(file.1);

        let output = StdCommand::new("ansible-playbook")
            .arg("--syntax-check")
            .arg(&file.0)
            .output()
            .expect("failed to run ansible-playbook");

        assert!(
            output.status.success(),
            "generated playbook failed --syntax-check:\nstdout: {}\nstderr: {}\n---\n{playbook}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn tempfile_path() -> (PathBuf, std::fs::File) {
        let path =
            std::env::temp_dir().join(format!("rdm-playbook-test-{}.yml", uuid::Uuid::new_v4()));
        let file = std::fs::File::create(&path).unwrap();
        (path, file)
    }

    #[test]
    fn playbook_with_empty_profile_packages_is_valid_yaml() {
        let hash = hash_password("s3cret!").unwrap();
        let playbook = build_playbook(
            "dev",
            &hash,
            "ssh-ed25519 AAAA... key",
            "vncpass",
            &[],
            None,
        );
        syntax_check(&playbook);
    }

    #[test]
    fn playbook_with_profile_packages_and_clone_is_valid_yaml() {
        let hash = hash_password("s3cret!").unwrap();
        let clone = CloneSpec {
            url: "https://x-access-token:tok@github.com/example/repo.git".to_string(),
            branch: "main".to_string(),
            dir_name: "repo".to_string(),
        };
        let playbook = build_playbook(
            "dev",
            &hash,
            "ssh-ed25519 AAAA... key",
            "vncpass",
            &["openjdk-17-jdk", "gradle"],
            Some(&clone),
        );
        syntax_check(&playbook);
    }
}
