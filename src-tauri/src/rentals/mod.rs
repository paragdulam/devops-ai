use crate::ansible;
use crate::aws;
use crate::github;
use crate::project_kind::Ide;
use crate::ssh;
use aws_sdk_ec2::Client as Ec2Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;

/// The GitHub repo picked when starting a rental, carried on the record so
/// `provision()` knows what to clone onto the instance. `clone_url` is the
/// unauthenticated URL from the GitHub API, kept only for reference/display —
/// the actual clone command is built with a token-embedded URL instead.
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GithubRepoSelection {
    pub github_account_id: String,
    pub repo_name: String,
    pub full_name: String,
    pub clone_url: String,
    pub default_branch: String,
}

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum RentalStatus {
    Requested,
    Provisioning,
    Booting,
    Connecting,
    Ready,
    Running,
    Stopping,
    Released,
    Failed,
}

#[derive(Clone)]
pub struct RentalRecord {
    pub id: String,
    pub status: RentalStatus,
    pub account_id: String,
    pub machine_profile: String,
    pub project_name: String,
    pub github_repo: Option<GithubRepoSelection>,
    pub vm_username: String,
    pub ec2_instance_id: Option<String>,
    pub security_group_id: Option<String>,
    pub key_pair_name: Option<String>,
    pub public_ip: Option<String>,
    pub vnc_password: Option<String>,
    pub provisioning_log: Vec<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub stopped_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RentalConnection {
    pub public_ip: String,
    pub vnc_port: u16,
    pub vnc_password: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RentalDto {
    pub id: String,
    pub status: RentalStatus,
    pub ec2_instance_id: Option<String>,
    pub machine_profile: String,
    pub project_name: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub stopped_at: Option<String>,
    pub error: Option<String>,
    pub connection: Option<RentalConnection>,
}

impl RentalRecord {
    pub fn to_dto(&self) -> RentalDto {
        let connection = match (&self.public_ip, &self.vnc_password) {
            (Some(ip), Some(password)) => Some(RentalConnection {
                public_ip: ip.clone(),
                vnc_port: aws::network::NOVNC_PORT as u16,
                vnc_password: password.clone(),
            }),
            _ => None,
        };

        RentalDto {
            id: self.id.clone(),
            status: self.status.clone(),
            ec2_instance_id: self.ec2_instance_id.clone(),
            machine_profile: self.machine_profile.clone(),
            project_name: self.project_name.clone(),
            created_at: self.created_at.clone(),
            started_at: self.started_at.clone(),
            stopped_at: self.stopped_at.clone(),
            error: self.error.clone(),
            connection,
        }
    }
}

#[derive(Default)]
pub struct RentalsState(pub Mutex<HashMap<String, RentalRecord>>);

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn state(app: &AppHandle) -> tauri::State<'_, RentalsState> {
    app.state::<RentalsState>()
}

async fn update_record<F: FnOnce(&mut RentalRecord)>(app: &AppHandle, id: &str, f: F) {
    let s = state(app);
    let mut map = s.0.lock().await;
    if let Some(record) = map.get_mut(id) {
        f(record);
    }
}

async fn set_status(app: &AppHandle, id: &str, status: RentalStatus) {
    update_record(app, id, |r| r.status = status).await;
}

pub async fn get_record(app: &AppHandle, id: &str) -> Option<RentalRecord> {
    let s = state(app);
    let map = s.0.lock().await;
    map.get(id).cloned()
}

pub async fn get_provisioning_log(app: &AppHandle, id: &str) -> Vec<String> {
    get_record(app, id)
        .await
        .map(|r| r.provisioning_log)
        .unwrap_or_default()
}

/// Appends one line to the in-memory replay buffer the on-demand log viewer
/// reads via `get_provisioning_log` — called by `ansible::provision` as each
/// line streams in, alongside the `rental://{id}/log` event it emits.
pub async fn append_log_line(app: &AppHandle, id: &str, line: String) {
    update_record(app, id, |r| r.provisioning_log.push(line)).await;
}

async fn set_failed(app: &AppHandle, id: &str, error: String) {
    eprintln!("rental {id} failed: {error}");
    update_record(app, id, |r| {
        r.status = RentalStatus::Failed;
        r.error = Some(error);
    })
    .await;
}

/// Terminate-then-delete-security-group, in that order (a security group
/// can't be deleted while still attached to a terminating ENI), then release
/// the SSH keypair (AWS-side and keyring-side) and the generated Ansible
/// artifacts. Best-effort throughout: used identically for an explicit Stop
/// and for a failed provisioning attempt, so nothing billed or exposed is
/// ever left behind.
async fn cleanup(
    app: &AppHandle,
    ec2: &Ec2Client,
    id: &str,
    instance_id: Option<&str>,
    security_group_id: Option<&str>,
    key_pair_name: Option<&str>,
) {
    if let Some(instance_id) = instance_id {
        if let Err(e) = aws::instance::terminate_instance(ec2, instance_id).await {
            eprintln!("cleanup: {e}");
        }
        if let Err(e) =
            aws::instance::wait_for_terminated(ec2, instance_id, Duration::from_secs(180)).await
        {
            eprintln!("cleanup: {e}");
        }
    }
    if let Some(sg_id) = security_group_id {
        if let Err(e) = aws::network::delete_security_group(ec2, sg_id).await {
            eprintln!("cleanup: {e}");
        }
    }
    if let Some(key_name) = key_pair_name {
        if let Err(e) = aws::keypair::delete_key_pair(ec2, key_name).await {
            eprintln!("cleanup: {e}");
        }
    }
    ssh::delete_stored_key(id);
    ansible::remove_artifacts(app, id);
}

#[allow(clippy::too_many_arguments)]
async fn cleanup_and_fail(
    app: &AppHandle,
    ec2: &Ec2Client,
    id: &str,
    instance_id: Option<&str>,
    security_group_id: Option<&str>,
    key_pair_name: Option<&str>,
    error: String,
) {
    cleanup(app, ec2, id, instance_id, security_group_id, key_pair_name).await;
    set_failed(app, id, error).await;
}

#[allow(clippy::too_many_arguments)]
pub fn new_record(
    id: String,
    account_id: String,
    machine_profile: String,
    project_name: String,
    vm_username: String,
    github_repo: Option<GithubRepoSelection>,
) -> RentalRecord {
    RentalRecord {
        id,
        status: RentalStatus::Requested,
        account_id,
        machine_profile,
        project_name,
        github_repo,
        vm_username,
        ec2_instance_id: None,
        security_group_id: None,
        key_pair_name: None,
        public_ip: None,
        vnc_password: None,
        provisioning_log: Vec::new(),
        created_at: now_iso(),
        started_at: None,
        stopped_at: None,
        error: None,
    }
}

/// The full provisioning workflow, run as a spawned background task.
/// Advances the rental through the same `RentalStatus` states the mock used.
pub async fn provision(
    app: AppHandle,
    id: String,
    account_id: String,
    machine_profile: String,
    vm_username: String,
    vm_password: String,
    ides: Vec<Ide>,
) {
    // Ansible cannot run as a control node on native Windows at all (an
    // upstream limitation, not something bundling could fix) — fail
    // immediately with an explicit message rather than an opaque
    // "ansible-playbook not found" error deep inside the preflight check.
    if cfg!(target_os = "windows") {
        set_failed(
            &app,
            &id,
            "Remote Dev Machine requires macOS or Linux (Ansible has no Windows control-node support)"
                .to_string(),
        )
        .await;
        return;
    }

    if let Err(e) = ansible::preflight().await {
        set_failed(&app, &id, e).await;
        return;
    }

    let (account, secret) =
        match crate::commands::accounts::resolve_account_secret(&app, &account_id) {
            Ok(v) => v,
            Err(e) => {
                set_failed(&app, &id, e).await;
                return;
            }
        };

    let profile = match aws::instance::profile_for(&machine_profile) {
        Ok(p) => p,
        Err(e) => {
            set_failed(&app, &id, e).await;
            return;
        }
    };

    set_status(&app, &id, RentalStatus::Provisioning).await;

    let sdk_config =
        aws::credentials::build_sdk_config(&account.region, &account.access_key_id, &secret).await;
    let ec2 = Ec2Client::new(&sdk_config);

    let (ami_id, root_device_name) = match aws::ami::resolve_ubuntu_ami(&ec2).await {
        Ok(v) => v,
        Err(e) => {
            set_failed(&app, &id, e).await;
            return;
        }
    };

    let (vpc_id, subnet_id) = match aws::network::resolve_default_vpc_and_subnet(&ec2).await {
        Ok(v) => v,
        Err(e) => {
            set_failed(&app, &id, e).await;
            return;
        }
    };

    let caller_ip = match aws::network::lookup_caller_ip().await {
        Ok(v) => v,
        Err(e) => {
            set_failed(&app, &id, e).await;
            return;
        }
    };

    let security_group_id =
        match aws::network::create_scoped_security_group(&ec2, &vpc_id, &id, &caller_ip).await {
            Ok(v) => v,
            Err(e) => {
                set_failed(&app, &id, e).await;
                return;
            }
        };
    update_record(&app, &id, |r| {
        r.security_group_id = Some(security_group_id.clone())
    })
    .await;

    let (public_key, private_key_pem) = match aws::keypair::generate_keypair() {
        Ok(v) => v,
        Err(e) => {
            cleanup_and_fail(&app, &ec2, &id, None, Some(&security_group_id), None, e).await;
            return;
        }
    };
    if let Err(e) = ssh::store_private_key(&id, &private_key_pem) {
        cleanup_and_fail(&app, &ec2, &id, None, Some(&security_group_id), None, e).await;
        return;
    }
    let key_pair_name = match aws::keypair::import_key_pair(&ec2, &id, &public_key).await {
        Ok(v) => v,
        Err(e) => {
            cleanup_and_fail(&app, &ec2, &id, None, Some(&security_group_id), None, e).await;
            return;
        }
    };
    update_record(&app, &id, |r| r.key_pair_name = Some(key_pair_name.clone())).await;

    // Diagnostic for "SSH authentication was rejected": lets the public key
    // this rental generated and imported be compared directly against
    // whatever the instance's own boot log (`ci-info: Authorized keys...`)
    // says actually landed in ~ubuntu/.ssh/authorized_keys. Not sensitive —
    // it's the public half.
    let key_log_line = format!("Generated and imported public key: {public_key}");
    let _ = app.emit(&format!("rental://{id}/log"), &key_log_line);
    append_log_line(&app, &id, key_log_line).await;

    let github_repo = get_record(&app, &id).await.and_then(|r| r.github_repo);
    let clone_spec = match github_repo {
        Some(repo) => match github::resolve_github_token(&repo.github_account_id) {
            Ok(token) => Some(ansible::CloneSpec {
                url: format!(
                    "https://x-access-token:{token}@github.com/{}.git",
                    repo.full_name
                ),
                branch: repo.default_branch,
                dir_name: repo.repo_name,
            }),
            Err(e) => {
                cleanup_and_fail(
                    &app,
                    &ec2,
                    &id,
                    None,
                    Some(&security_group_id),
                    Some(&key_pair_name),
                    format!("Could not resolve GitHub token: {e}"),
                )
                .await;
                return;
            }
        },
        None => None,
    };

    let vnc_password = ssh::generate_vnc_password();

    let instance_id = match aws::instance::launch_instance(
        &ec2,
        &ami_id,
        &root_device_name,
        profile.root_volume_gb,
        profile.instance_type,
        &subnet_id,
        &security_group_id,
        &id,
        &key_pair_name,
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            cleanup_and_fail(
                &app,
                &ec2,
                &id,
                None,
                Some(&security_group_id),
                Some(&key_pair_name),
                e,
            )
            .await;
            return;
        }
    };
    update_record(&app, &id, |r| {
        r.ec2_instance_id = Some(instance_id.clone());
        r.vnc_password = Some(vnc_password.clone());
    })
    .await;

    let public_ip =
        match aws::instance::wait_for_running(&ec2, &instance_id, Duration::from_secs(180)).await {
            Ok(ip) => ip,
            Err(e) => {
                cleanup_and_fail(
                    &app,
                    &ec2,
                    &id,
                    Some(&instance_id),
                    Some(&security_group_id),
                    Some(&key_pair_name),
                    e,
                )
                .await;
                return;
            }
        };
    update_record(&app, &id, |r| r.public_ip = Some(public_ip.clone())).await;
    set_status(&app, &id, RentalStatus::Booting).await;

    // EC2's own stock-AMI first-boot behavior drops the imported public key
    // into ~ubuntu/.ssh/authorized_keys — inherent AMI behavior, not
    // something this app controls or can query for completion. A real SSH
    // auth handshake (not just a TCP probe) is the true readiness signal,
    // since a TCP-open port during early sshd startup can still reject auth.
    if let Err(e) =
        aws::instance::wait_for_ssh_ready(&public_ip, &private_key_pem, Duration::from_secs(300))
            .await
    {
        cleanup_and_fail(
            &app,
            &ec2,
            &id,
            Some(&instance_id),
            Some(&security_group_id),
            Some(&key_pair_name),
            e,
        )
        .await;
        return;
    }

    set_status(&app, &id, RentalStatus::Connecting).await;

    let key_path = match ssh::materialize_key_file(&app, &id) {
        Ok(p) => p,
        Err(e) => {
            cleanup_and_fail(
                &app,
                &ec2,
                &id,
                Some(&instance_id),
                Some(&security_group_id),
                Some(&key_pair_name),
                e,
            )
            .await;
            return;
        }
    };

    let ansible_result = ansible::provision(ansible::ProvisionParams {
        app: &app,
        rental_id: &id,
        host: &public_ip,
        private_key_path: &key_path,
        vm_username: &vm_username,
        vm_password: &vm_password,
        authorized_key_line: &public_key,
        vnc_password: &vnc_password,
        profile_packages: profile.ansible_packages,
        ides: &ides,
        clone: clone_spec.as_ref(),
    })
    .await;
    ssh::remove_key_file(&key_path);

    if let Err(e) = ansible_result {
        cleanup_and_fail(
            &app,
            &ec2,
            &id,
            Some(&instance_id),
            Some(&security_group_id),
            Some(&key_pair_name),
            e,
        )
        .await;
        return;
    }

    if let Err(e) = aws::instance::wait_for_port_open(
        &public_ip,
        aws::network::NOVNC_PORT as u16,
        Duration::from_secs(180),
    )
    .await
    {
        cleanup_and_fail(
            &app,
            &ec2,
            &id,
            Some(&instance_id),
            Some(&security_group_id),
            Some(&key_pair_name),
            e,
        )
        .await;
        return;
    }

    set_status(&app, &id, RentalStatus::Ready).await;
    update_record(&app, &id, |r| r.started_at = Some(now_iso())).await;
    set_status(&app, &id, RentalStatus::Running).await;
}

/// Terminates the instance and revokes the security group and keypair, then
/// marks the rental RELEASED. Used for an explicit Stop Renting / Disconnect.
pub async fn stop(app: AppHandle, id: String) {
    let Some(record) = get_record(&app, &id).await else {
        return;
    };
    if matches!(record.status, RentalStatus::Released) {
        return;
    }

    let (account, secret) =
        match crate::commands::accounts::resolve_account_secret(&app, &record.account_id) {
            Ok(v) => v,
            Err(e) => {
                set_failed(&app, &id, format!("Could not stop rental: {e}")).await;
                return;
            }
        };
    let sdk_config =
        aws::credentials::build_sdk_config(&account.region, &account.access_key_id, &secret).await;
    let ec2 = Ec2Client::new(&sdk_config);

    cleanup(
        &app,
        &ec2,
        &id,
        record.ec2_instance_id.as_deref(),
        record.security_group_id.as_deref(),
        record.key_pair_name.as_deref(),
    )
    .await;

    update_record(&app, &id, |r| r.stopped_at = Some(now_iso())).await;
    set_status(&app, &id, RentalStatus::Released).await;
}
