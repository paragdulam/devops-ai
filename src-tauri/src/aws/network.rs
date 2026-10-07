use aws_sdk_ec2::types::{Filter, IpPermission, IpRange};
use aws_sdk_ec2::Client;
use std::time::Duration;

pub const NOVNC_PORT: i32 = 6080;
pub const SSH_PORT: i32 = 22;

/// Looks up the current machine's public IP so the security group can be
/// scoped to it — never opened to 0.0.0.0/0.
pub async fn lookup_caller_ip() -> Result<String, String> {
    let body = reqwest::get("https://checkip.amazonaws.com")
        .await
        .map_err(|e| format!("Could not determine public IP: {e}"))?
        .text()
        .await
        .map_err(|e| format!("Could not read public IP response: {e}"))?;
    Ok(body.trim().to_string())
}

/// Prefers the account's default VPC, but falls back to any available VPC in
/// the region — some accounts (e.g. ones that never had a default VPC, or
/// had it deleted) have none, and this launch path shouldn't hard-fail just
/// because that one's missing.
pub async fn resolve_default_vpc_and_subnet(ec2: &Client) -> Result<(String, String), String> {
    let default_vpcs = ec2
        .describe_vpcs()
        .filters(Filter::builder().name("isDefault").values("true").build())
        .send()
        .await
        .map_err(|e| format!("Could not describe VPCs: {e:?}"))?;

    let vpc_id = match default_vpcs.vpcs().first().and_then(|v| v.vpc_id()) {
        Some(id) => id.to_string(),
        None => {
            let all_vpcs = ec2
                .describe_vpcs()
                .send()
                .await
                .map_err(|e| format!("Could not describe VPCs: {e:?}"))?;
            all_vpcs
                .vpcs()
                .first()
                .and_then(|v| v.vpc_id())
                .ok_or_else(|| {
                    "No VPCs found in this region/account — at least one VPC is needed to launch into"
                        .to_string()
                })?
                .to_string()
        }
    };

    let subnets = ec2
        .describe_subnets()
        .filters(
            Filter::builder()
                .name("vpc-id")
                .values(vpc_id.clone())
                .build(),
        )
        .send()
        .await
        .map_err(|e| format!("Could not describe subnets: {e:?}"))?;

    let subnet_id = subnets
        .subnets()
        .first()
        .and_then(|s| s.subnet_id())
        .ok_or_else(|| format!("No subnets found in VPC {vpc_id}"))?
        .to_string();

    Ok((vpc_id, subnet_id))
}

/// Authorizes a single TCP port, scoped to the caller's own `/32` — never
/// widen any of these to 0.0.0.0/0.
async fn authorize_port(
    ec2: &Client,
    group_id: &str,
    port: i32,
    caller_ip: &str,
) -> Result<(), String> {
    let cidr = format!("{caller_ip}/32");
    ec2.authorize_security_group_ingress()
        .group_id(group_id)
        .ip_permissions(
            IpPermission::builder()
                .ip_protocol("tcp")
                .from_port(port)
                .to_port(port)
                .ip_ranges(IpRange::builder().cidr_ip(cidr).build())
                .build(),
        )
        .send()
        .await
        .map_err(|e| format!("Could not authorize security group ingress on port {port}: {e:?}"))?;
    Ok(())
}

/// Creates a security group opening the noVNC port and the SSH port, both
/// scoped ONLY to the caller's current IP. SSH is opened deliberately, so
/// this app's own Ansible provisioning (and the on-demand terminal) can
/// reach the instance — never widen either rule to 0.0.0.0/0.
pub async fn create_scoped_security_group(
    ec2: &Client,
    vpc_id: &str,
    rental_id: &str,
    caller_ip: &str,
) -> Result<String, String> {
    let created = ec2
        .create_security_group()
        .group_name(format!("rdm-{rental_id}"))
        .description("Remote Dev Machine rental - noVNC and SSH access scoped to one caller IP")
        .vpc_id(vpc_id)
        .send()
        .await
        .map_err(|e| format!("Could not create security group: {e:?}"))?;

    let group_id = created
        .group_id()
        .ok_or_else(|| "CreateSecurityGroup returned no group id".to_string())?
        .to_string();

    authorize_port(ec2, &group_id, NOVNC_PORT, caller_ip).await?;
    authorize_port(ec2, &group_id, SSH_PORT, caller_ip).await?;

    Ok(group_id)
}

/// Security group deletion fails while it's still attached to a terminating
/// ENI, so retry with backoff instead of treating the first failure as final.
pub async fn delete_security_group(ec2: &Client, group_id: &str) -> Result<(), String> {
    let mut last_err = String::new();
    for attempt in 0..8 {
        match ec2.delete_security_group().group_id(group_id).send().await {
            Ok(_) => return Ok(()),
            Err(e) => {
                last_err = e.to_string();
                tokio::time::sleep(Duration::from_secs(3 + attempt)).await;
            }
        }
    }
    Err(format!(
        "Could not delete security group {group_id} after retries: {last_err}"
    ))
}

/// Re-scopes the group's SSH and noVNC rules to `caller_ip` — the group is
/// created with the caller's IP at rental start, so a changed IP (new
/// network, VPN) would otherwise lock the user out of a running rental.
/// Existing rules for those two ports are revoked first; the rules stay
/// scoped to a single `/32`. A no-op when already scoped to `caller_ip`.
pub async fn rescope_security_group(
    ec2: &Client,
    group_id: &str,
    caller_ip: &str,
) -> Result<(), String> {
    let wanted = format!("{caller_ip}/32");
    let described = ec2
        .describe_security_groups()
        .group_ids(group_id)
        .send()
        .await
        .map_err(|e| format!("Could not describe security group {group_id}: {e:?}"))?;

    let mut stale = Vec::new();
    let mut current_ports = Vec::new();
    for group in described.security_groups() {
        for perm in group.ip_permissions() {
            let Some(port) = perm.from_port() else {
                continue;
            };
            if port != NOVNC_PORT && port != SSH_PORT {
                continue;
            }
            let cidrs: Vec<&str> = perm
                .ip_ranges()
                .iter()
                .filter_map(|r| r.cidr_ip())
                .collect();
            if cidrs == [wanted.as_str()] {
                current_ports.push(port);
            } else {
                stale.push(perm.clone());
            }
        }
    }

    if !stale.is_empty() {
        ec2.revoke_security_group_ingress()
            .group_id(group_id)
            .set_ip_permissions(Some(stale))
            .send()
            .await
            .map_err(|e| format!("Could not revoke stale ingress rules: {e:?}"))?;
    }
    for port in [NOVNC_PORT, SSH_PORT] {
        if !current_ports.contains(&port) {
            authorize_port(ec2, group_id, port, caller_ip).await?;
        }
    }
    Ok(())
}
