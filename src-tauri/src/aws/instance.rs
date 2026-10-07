use aws_sdk_ec2::types::{
    BlockDeviceMapping, EbsBlockDevice, InstanceNetworkInterfaceSpecification, InstanceStateName,
    InstanceType, ResourceType, Tag, TagSpecification, VolumeType,
};
use aws_sdk_ec2::Client;
use std::net::ToSocketAddrs;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;

pub struct MachineProfile {
    pub id: &'static str,
    pub instance_type: &'static str,
    /// Extra `apt` packages installed by the Ansible playbook for this
    /// profile, on top of the base desktop/VNC package set. Empty for
    /// `"standard"` today — a hook for later profiles (e.g. Android dev
    /// tooling), not a catalog UI.
    pub ansible_packages: &'static [&'static str],
    /// Root volume size in GiB. Canonical's Ubuntu AMIs default to 8 GiB,
    /// which isn't enough once a cloned repo, its build artifacts, and any
    /// package manager caches land on the same disk.
    pub root_volume_gb: i32,
}

const PROFILES: &[MachineProfile] = &[MachineProfile {
    id: "standard",
    instance_type: "m5.2xlarge",
    ansible_packages: &[],
    root_volume_gb: 100,
}];

pub fn profile_for(id: &str) -> Result<&'static MachineProfile, String> {
    PROFILES
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("Unknown machine profile: {id}"))
}

pub async fn launch_instance(
    ec2: &Client,
    ami_id: &str,
    root_device_name: &str,
    root_volume_gb: i32,
    instance_type: &str,
    subnet_id: &str,
    security_group_id: &str,
    rental_id: &str,
    key_name: &str,
) -> Result<String, String> {
    let nic = InstanceNetworkInterfaceSpecification::builder()
        .device_index(0)
        .subnet_id(subnet_id)
        .groups(security_group_id)
        .associate_public_ip_address(true)
        .build();

    let name_tag = Tag::builder()
        .key("Name")
        .value(format!("rdm-rental-{rental_id}"))
        .build();
    let rental_tag = Tag::builder()
        .key("remote-dev-machine-rental-id")
        .value(rental_id)
        .build();

    let instance_tags = TagSpecification::builder()
        .resource_type(ResourceType::Instance)
        .tags(name_tag.clone())
        .tags(rental_tag.clone())
        .build();

    // `RunInstances` also creates the root EBS volume, and some accounts
    // enforce a tag condition on resource creation (e.g. "every created
    // resource must carry this tag"). Tagging only the instance leaves the
    // volume untagged, which can make that condition deny just the volume
    // half of the launch — the instance comes up, but its root volume never
    // actually gets created ("the volume ... does not exist" in the console).
    let volume_tags = TagSpecification::builder()
        .resource_type(ResourceType::Volume)
        .tags(name_tag)
        .tags(rental_tag)
        .build();

    // Overrides the AMI's default (8 GiB for stock Ubuntu images) — cloning a
    // repo plus its build artifacts/package caches doesn't fit in that.
    // `delete_on_termination` is set explicitly here rather than relying on
    // the AMI's own default, so the volume is guaranteed to go away with the
    // instance regardless of what any future AMI ships.
    let root_volume = BlockDeviceMapping::builder()
        .device_name(root_device_name)
        .ebs(
            EbsBlockDevice::builder()
                .volume_size(root_volume_gb)
                .volume_type(VolumeType::Gp3)
                .delete_on_termination(true)
                .build(),
        )
        .build();

    let output = ec2
        .run_instances()
        .image_id(ami_id)
        .instance_type(InstanceType::from(instance_type))
        .min_count(1)
        .max_count(1)
        .key_name(key_name)
        .network_interfaces(nic)
        .block_device_mappings(root_volume)
        .tag_specifications(instance_tags)
        .tag_specifications(volume_tags)
        .send()
        .await
        .map_err(|e| format!("Could not launch EC2 instance: {e:?}"))?;

    output
        .instances()
        .first()
        .and_then(|i| i.instance_id())
        .map(|id| id.to_string())
        .ok_or_else(|| "RunInstances returned no instance id".to_string())
}

/// Polls DescribeInstances until the instance is running and has a public IP,
/// or the time budget runs out.
pub async fn wait_for_running(
    ec2: &Client,
    instance_id: &str,
    budget: Duration,
) -> Result<String, String> {
    let deadline = Instant::now() + budget;
    loop {
        let output = ec2
            .describe_instances()
            .instance_ids(instance_id)
            .send()
            .await
            .map_err(|e| format!("Could not describe instance {instance_id}: {e:?}"))?;

        let instance = output
            .reservations()
            .first()
            .and_then(|r| r.instances().first());

        if let Some(instance) = instance {
            let state = instance.state().and_then(|s| s.name());
            if matches!(state, Some(InstanceStateName::Running)) {
                if let Some(ip) = instance.public_ip_address() {
                    return Ok(ip.to_string());
                }
            }
            if matches!(
                state,
                Some(InstanceStateName::Terminated) | Some(InstanceStateName::ShuttingDown)
            ) {
                return Err(format!(
                    "Instance {instance_id} entered unexpected state {:?} while waiting to run",
                    state
                ));
            }
        }

        if Instant::now() >= deadline {
            return Err(format!(
                "Timed out waiting for instance {instance_id} to reach running with a public IP"
            ));
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

/// Polls a TCP connection to `(host, port)` until it succeeds or the time
/// budget runs out — used as the readiness signal for the websockify port.
pub async fn wait_for_port_open(host: &str, port: u16, budget: Duration) -> Result<(), String> {
    let deadline = Instant::now() + budget;
    let addr = format!("{host}:{port}");
    loop {
        let socket_addrs: Vec<_> = addr
            .to_socket_addrs()
            .map_err(|e| format!("Could not resolve {addr}: {e}"))?
            .collect();

        for socket_addr in &socket_addrs {
            if tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(socket_addr))
                .await
                .map(|r| r.is_ok())
                .unwrap_or(false)
            {
                return Ok(());
            }
        }

        if Instant::now() >= deadline {
            return Err(format!("Timed out waiting for {addr} to become reachable"));
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// Retrying readiness probe combining a TCP connect on port 22 with a real
/// SSH auth handshake using the rental's imported keypair — a TCP-open port
/// during early `sshd` startup can still reject auth for a few more seconds,
/// so the auth attempt itself (open + immediately close a session) is the
/// true readiness signal, not just the socket. This is what makes EC2's
/// inherent first-boot key injection into `~ubuntu/.ssh/authorized_keys`
/// (not something this app controls) observable as "done."
pub async fn wait_for_ssh_ready(
    host: &str,
    private_key_pem: &str,
    budget: Duration,
) -> Result<(), String> {
    let deadline = Instant::now() + budget;
    let mut last_err = String::new();
    loop {
        match crate::ssh::connect(host, "ubuntu", private_key_pem).await {
            Ok(handle) => {
                drop(handle);
                return Ok(());
            }
            Err(e) => last_err = e,
        }

        if Instant::now() >= deadline {
            return Err(format!(
                "Timed out waiting for SSH to become ready on {host}: {last_err}"
            ));
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

pub async fn terminate_instance(ec2: &Client, instance_id: &str) -> Result<(), String> {
    ec2.terminate_instances()
        .instance_ids(instance_id)
        .send()
        .await
        .map_err(|e| format!("Could not terminate instance {instance_id}: {e:?}"))?;
    Ok(())
}

pub async fn wait_for_terminated(
    ec2: &Client,
    instance_id: &str,
    budget: Duration,
) -> Result<(), String> {
    let deadline = Instant::now() + budget;
    loop {
        let output = ec2
            .describe_instances()
            .instance_ids(instance_id)
            .send()
            .await
            .map_err(|e| format!("Could not describe instance {instance_id}: {e:?}"))?;

        let state = output
            .reservations()
            .first()
            .and_then(|r| r.instances().first())
            .and_then(|i| i.state())
            .and_then(|s| s.name())
            .cloned();

        if matches!(state, Some(InstanceStateName::Terminated)) {
            return Ok(());
        }

        if Instant::now() >= deadline {
            return Err(format!(
                "Timed out waiting for instance {instance_id} to terminate"
            ));
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

pub enum Liveness {
    /// Running, with its current public IP.
    Running(String),
    /// Terminated, shutting down, stopped, or unknown to EC2 — nothing to
    /// reattach to.
    Gone,
}

/// One-shot DescribeInstances used when a restarted app reattaches to a
/// persisted rental. A transport/API error is returned as `Err` so the caller
/// can leave the rental untouched rather than wrongly releasing it.
pub async fn check_liveness(ec2: &Client, instance_id: &str) -> Result<Liveness, String> {
    let output = match ec2
        .describe_instances()
        .instance_ids(instance_id)
        .send()
        .await
    {
        Ok(o) => o,
        Err(e) => {
            // A terminated instance eventually ages out of DescribeInstances.
            if e.to_string().contains("InvalidInstanceID") {
                return Ok(Liveness::Gone);
            }
            return Err(format!("Could not describe instance {instance_id}: {e:?}"));
        }
    };
    let instance = output
        .reservations()
        .first()
        .and_then(|r| r.instances().first());
    match instance {
        Some(i)
            if matches!(
                i.state().and_then(|s| s.name()),
                Some(InstanceStateName::Running)
            ) =>
        {
            Ok(i.public_ip_address()
                .map(|ip| Liveness::Running(ip.to_string()))
                .unwrap_or(Liveness::Gone))
        }
        _ => Ok(Liveness::Gone),
    }
}
