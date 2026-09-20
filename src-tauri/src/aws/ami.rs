use aws_sdk_ec2::types::Filter;
use aws_sdk_ec2::Client;

/// Canonical's official AMI-publishing account — images owned by this account
/// are the authoritative source for stock Ubuntu AMIs (same account the AWS
/// SSM `/aws/service/canonical/...` parameters ultimately point at).
const CANONICAL_OWNER_ID: &str = "099720109477";
const UBUNTU_2204_NAME_FILTER: &str = "ubuntu/images/hvm-ssd-gp3/ubuntu-jammy-22.04-amd64-server-*";

/// Resolves the current Ubuntu 22.04 AMI id for the client's region via
/// `DescribeImages` against Canonical's account, rather than hardcoding an
/// AMI id that would go stale or not exist in every region.
///
/// Deliberately not `ssm:GetParameter` (the more common convenience lookup
/// for this) — some AWS Organizations attach a Service Control Policy that
/// explicitly denies SSM Parameter Store reads regardless of the caller's
/// own IAM permissions, which made this unusable for at least one real
/// account. `DescribeImages` reaches the same "always current, never
/// hardcoded" goal through a different, commonly-unrestricted API.
pub async fn resolve_ubuntu_ami(ec2: &Client) -> Result<String, String> {
    let output = ec2
        .describe_images()
        .owners(CANONICAL_OWNER_ID)
        .filters(Filter::builder().name("name").values(UBUNTU_2204_NAME_FILTER).build())
        .filters(Filter::builder().name("state").values("available").build())
        .filters(Filter::builder().name("architecture").values("x86_64").build())
        .filters(Filter::builder().name("virtualization-type").values("hvm").build())
        .send()
        .await
        .map_err(|e| format!("Could not resolve Ubuntu AMI: {e:?}"))?;

    output
        .images()
        .iter()
        .max_by_key(|i| i.creation_date().unwrap_or("").to_string())
        .and_then(|i| i.image_id())
        .map(|v| v.to_string())
        .ok_or_else(|| "No matching Ubuntu 22.04 AMI found via DescribeImages".to_string())
}
