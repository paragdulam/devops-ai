use aws_sdk_ec2::types::{
    InstanceNetworkInterfaceSpecification, InstanceStateName, InstanceType, ResourceType, Tag,
    TagSpecification,
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
}

const PROFILES: &[MachineProfile] = &[MachineProfile {
    id: "standard",
    instance_type: "m5.2xlarge",
    ansible_packages: &[],
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

    let tags = TagSpecification::builder()
        .resource_type(ResourceType::Instance)
        .tags(Tag::builder().key("Name").value(format!("rdm-rental-{rental_id}")).build())
        .tags(Tag::builder().key("remote-dev-machine-rental-id").value(rental_id).build())
        .build();

    let output = ec2
        .run_instances()
        .image_id(ami_id)
        .instance_type(InstanceType::from(instance_type))
        .min_count(1)
        .max_count(1)
        .key_name(key_name)
        .network_interfaces(nic)
        .tag_specifications(tags)
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
