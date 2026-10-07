use crate::ansible;
use crate::aws;
use crate::github;
use crate::project_kind::Ide;
use crate::ssh;
use crate::toolchain::ToolRequirement;
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

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
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

/// Persisted to `rentals.json` (minus the log and the VNC password, which
/// lives in the keyring) so running VMs can be re-attached after a restart.
#[derive(Clone, Serialize, Deserialize)]
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
    /// When the instance was launched — billing starts here, not at
    /// `started_at` (set only once the desktop is ready, minutes later).
    #[serde(default)]
    pub launched_at: Option<String>,
    /// All-in USD/hour (instance + disk + public IPv4), fixed at launch.
    #[serde(default)]
    pub hourly_rate_usd: Option<f64>,
    #[serde(default)]
    pub rate_source: Option<String>,
    #[serde(skip)]
    pub vnc_password: Option<String>,
    #[serde(skip)]
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
    pub github_repo: Option<GithubRepoSelection>,
    pub launched_at: Option<String>,
    pub hourly_rate_usd: Option<f64>,
    pub rate_source: Option<String>,
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
            github_repo: self.github_repo.clone(),
            launched_at: self.launched_at.clone(),
            hourly_rate_usd: self.hourly_rate_usd,
            rate_source: self.rate_source.clone(),
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

const RENTALS_FILE: &str = "rentals.json";

fn rentals_file_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(RENTALS_FILE))
}

/// Records with nothing left to reattach to (Released / Failed — both only
/// reached after cleanup) are not persisted.
fn is_persistable(record: &RentalRecord) -> bool {
    !matches!(record.status, RentalStatus::Released | RentalStatus::Failed)
}

/// Best-effort atomic write (tmp file + rename) — a failed write only costs
/// reattach-after-restart, so it is logged rather than propagated.
pub fn persist(app: &AppHandle, map: &HashMap<String, RentalRecord>) {
    let result = (|| -> Result<(), String> {
        let path = rentals_file_path(app)?;
        let records: Vec<&RentalRecord> = map.values().filter(|r| is_persistable(r)).collect();
        let raw = serde_json::to_string_pretty(&records).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, raw).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
    })();
    if let Err(e) = result {
        eprintln!("could not persist rentals: {e}");
    }
}

async fn update_record<F: FnOnce(&mut RentalRecord)>(app: &AppHandle, id: &str, f: F) {
    let s = state(app);
    let mut map = s.0.lock().await;
    if let Some(record) = map.get_mut(id) {
        f(record);
    }
    persist(app, &map);
}

pub async fn insert_record(app: &AppHandle, record: RentalRecord) {
    let s = state(app);
    let mut map = s.0.lock().await;
    map.insert(record.id.clone(), record);
    persist(app, &map);
}

pub async fn list_dtos(app: &AppHandle) -> Vec<RentalDto> {
    let s = state(app);
    let map = s.0.lock().await;
    let mut dtos: Vec<RentalDto> = map.values().map(|r| r.to_dto()).collect();
    dtos.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    dtos
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
    // Not `update_record`: the log isn't persisted, so skip the file write
    // that would otherwise happen on every streamed line.
    let s = state(app);
    let mut map = s.0.lock().await;
    if let Some(record) = map.get_mut(id) {
        record.provisioning_log.push(line);
    }
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
    ssh::delete_vnc_password(id);
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
        launched_at: None,
        hourly_rate_usd: None,
        rate_source: None,
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
    tools: Vec<ToolRequirement>,
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
    if let Err(e) = ssh::store_vnc_password(&id, &vnc_password) {
        eprintln!("could not store VNC password for {id}: {e}");
    }
    // A pricing failure must never fail the rental — it only means no cost
    // estimate is shown.
    let rate = aws::pricing::hourly_rate(
        &sdk_config,
        &account.region,
        profile.instance_type,
        profile.root_volume_gb,
    )
    .await
    .map_err(|e| eprintln!("rental {id}: no cost estimate: {e}"))
    .ok();
    update_record(&app, &id, |r| {
        r.ec2_instance_id = Some(instance_id.clone());
        r.vnc_password = Some(vnc_password.clone());
        r.launched_at = Some(now_iso());
        r.hourly_rate_usd = rate.map(|r| r.total());
        r.rate_source = rate.map(|r| r.source.as_str().to_string());
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
        tools: &tools,
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

async fn ec2_for_account(app: &AppHandle, account_id: &str) -> Result<Ec2Client, String> {
    let (account, secret) = crate::commands::accounts::resolve_account_secret(app, account_id)?;
    let sdk_config =
        aws::credentials::build_sdk_config(&account.region, &account.access_key_id, &secret).await;
    Ok(Ec2Client::new(&sdk_config))
}

/// Re-scopes the rental's security group to the caller's current public IP,
/// for when the user's IP changed (new network, VPN) after the rental began.
pub async fn refresh_access(app: &AppHandle, id: &str) -> Result<(), String> {
    let record = get_record(app, id)
        .await
        .ok_or_else(|| format!("No rental with id {id}"))?;
    let sg_id = record
        .security_group_id
        .ok_or_else(|| "This rental has no security group".to_string())?;
    let ec2 = ec2_for_account(app, &record.account_id).await?;
    let caller_ip = aws::network::lookup_caller_ip().await?;
    aws::network::rescope_security_group(&ec2, &sg_id, &caller_ip).await
}

/// Loads rentals persisted by a previous run into `RentalsState`, synchronously
/// so `list_rentals` never races it. A rental caught mid-provisioning can't be
/// resumed, so it is parked as STOPPING (with an error) for `reconcile` to
/// tear down. Returns the ids `reconcile` must check against AWS.
pub fn restore(app: &AppHandle) -> Vec<String> {
    let records: Vec<RentalRecord> = (|| -> Result<Vec<RentalRecord>, String> {
        let path = rentals_file_path(app)?;
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if raw.trim().is_empty() {
            return Ok(Vec::new());
        }
        serde_json::from_str(&raw).map_err(|e| e.to_string())
    })()
    .unwrap_or_else(|e| {
        eprintln!("could not load persisted rentals: {e}");
        Vec::new()
    });

    let s = state(app);
    let mut map = s.0.blocking_lock();
    let mut ids = Vec::new();
    for mut record in records {
        record.vnc_password = ssh::load_vnc_password(&record.id);
        if matches!(
            record.status,
            RentalStatus::Requested
                | RentalStatus::Provisioning
                | RentalStatus::Booting
                | RentalStatus::Connecting
        ) {
            record.status = RentalStatus::Stopping;
            record.error =
                Some("Interrupted by an app restart; its resources were released".into());
        }
        ids.push(record.id.clone());
        map.insert(record.id.clone(), record);
    }
    ids
}

/// Checks a restored rental against AWS: a still-running instance is kept
/// (and its security group re-scoped to the current IP); anything else is
/// cleaned up. API errors leave the rental untouched for the next launch.
pub async fn reconcile(app: AppHandle, id: String) {
    let Some(record) = get_record(&app, &id).await else {
        return;
    };
    let ec2 = match ec2_for_account(&app, &record.account_id).await {
        Ok(ec2) => ec2,
        Err(e) => {
            eprintln!("reconcile {id}: {e}");
            return;
        }
    };

    match record.status {
        RentalStatus::Ready | RentalStatus::Running => {
            let Some(instance_id) = record.ec2_instance_id.as_deref() else {
                return;
            };
            match aws::instance::check_liveness(&ec2, instance_id).await {
                Ok(aws::instance::Liveness::Running(ip)) => {
                    update_record(&app, &id, |r| r.public_ip = Some(ip)).await;
                    if let Some(sg_id) = record.security_group_id.as_deref() {
                        let rescoped = match aws::network::lookup_caller_ip().await {
                            Ok(ip) => aws::network::rescope_security_group(&ec2, sg_id, &ip).await,
                            Err(e) => Err(e),
                        };
                        if let Err(e) = rescoped {
                            eprintln!("reconcile {id}: {e}");
                        }
                    }
                }
                Ok(aws::instance::Liveness::Gone) => {
                    cleanup(
                        &app,
                        &ec2,
                        &id,
                        None,
                        record.security_group_id.as_deref(),
                        record.key_pair_name.as_deref(),
                    )
                    .await;
                    set_failed(
                        &app,
                        &id,
                        "The instance is no longer running (it was terminated outside the app)"
                            .into(),
                    )
                    .await;
                }
                Err(e) => eprintln!("reconcile {id}: {e}"),
            }
        }
        RentalStatus::Stopping => {
            cleanup(
                &app,
                &ec2,
                &id,
                record.ec2_instance_id.as_deref(),
                record.security_group_id.as_deref(),
                record.key_pair_name.as_deref(),
            )
            .await;
            match record.error {
                Some(error) => set_failed(&app, &id, error).await,
                None => {
                    update_record(&app, &id, |r| r.stopped_at = Some(now_iso())).await;
                    set_status(&app, &id, RentalStatus::Released).await;
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(status: RentalStatus) -> RentalRecord {
        let mut r = new_record(
            "r1".into(),
            "acct".into(),
            "standard".into(),
            "proj".into(),
            "dev".into(),
            None,
        );
        r.status = status;
        r.vnc_password = Some("secret".into());
        r.provisioning_log.push("line".into());
        r
    }

    #[test]
    fn cost_fields_survive_persistence_and_old_files_still_load() {
        let mut r = record(RentalStatus::Running);
        r.launched_at = Some("2026-10-01T00:00:00Z".into());
        r.hourly_rate_usd = Some(0.4);
        r.rate_source = Some("fallback".into());
        let back: RentalRecord = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(back.hourly_rate_usd, Some(0.4));
        assert_eq!(back.launched_at.as_deref(), Some("2026-10-01T00:00:00Z"));

        // A rentals.json written before these fields existed.
        let mut old: serde_json::Value = serde_json::to_value(&r).unwrap();
        for k in ["launched_at", "hourly_rate_usd", "rate_source"] {
            old.as_object_mut().unwrap().remove(k);
        }
        let back: RentalRecord = serde_json::from_value(old).unwrap();
        assert!(back.hourly_rate_usd.is_none());
    }

    #[test]
    fn persisted_json_omits_vnc_password_and_log() {
        let json = serde_json::to_string(&record(RentalStatus::Running)).unwrap();
        assert!(!json.contains("secret"));
        assert!(!json.contains("line"));
        let back: RentalRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back.status, RentalStatus::Running);
        assert!(back.vnc_password.is_none());
        assert!(back.provisioning_log.is_empty());
    }

    #[test]
    fn released_and_failed_records_are_not_persisted() {
        assert!(is_persistable(&record(RentalStatus::Running)));
        assert!(is_persistable(&record(RentalStatus::Stopping)));
        assert!(!is_persistable(&record(RentalStatus::Released)));
        assert!(!is_persistable(&record(RentalStatus::Failed)));
    }
}

/// How long a Cost Explorer answer is reused: each request is billed
/// ($0.01) and the underlying data only changes about daily.
const ACTUAL_COST_TTL: Duration = Duration::from_secs(3600);

#[derive(Default)]
pub struct ActualCostCache(pub Mutex<HashMap<String, (std::time::Instant, aws::cost::ActualCost)>>);

/// Billed cost of a rental from AWS Cost Explorer, cached per rental.
pub async fn actual_cost(
    app: &AppHandle,
    id: &str,
    force: bool,
) -> Result<aws::cost::ActualCost, String> {
    let cache = app.state::<ActualCostCache>();
    if !force {
        if let Some((at, cost)) = cache.0.lock().await.get(id) {
            if at.elapsed() < ACTUAL_COST_TTL {
                return Ok(cost.clone());
            }
        }
    }

    let record = get_record(app, id)
        .await
        .ok_or_else(|| format!("No rental with id {id}"))?;
    let Some(launched_at) = record
        .launched_at
        .as_deref()
        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
    else {
        return Ok(aws::cost::ActualCost::Unavailable {
            reason: "The instance hasn't launched yet".to_string(),
        });
    };

    let (account, secret) =
        crate::commands::accounts::resolve_account_secret(app, &record.account_id)?;
    let sdk_config =
        aws::credentials::build_sdk_config(&account.region, &account.access_key_id, &secret).await;
    let cost =
        aws::cost::rental_cost(&sdk_config, id, launched_at.with_timezone(&chrono::Utc)).await;

    // Only a real answer is worth reusing; an error should be retried.
    if matches!(cost, aws::cost::ActualCost::Available { .. }) {
        cache
            .0
            .lock()
            .await
            .insert(id.to_string(), (std::time::Instant::now(), cost.clone()));
    }
    Ok(cost)
}
