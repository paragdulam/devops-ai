use aws_sdk_ec2::types::Filter;
use aws_sdk_ec2::Client;

/// Canonical's official AMI-publishing account — images owned by this account
/// are the authoritative source for stock Ubuntu AMIs (same account the AWS
/// SSM `/aws/service/canonical/...` parameters ultimately point at).
const CANONICAL_OWNER_ID: &str = "099720109477";
// Jammy (22.04) predates the gp3-based image variant Canonical introduced for
// releases >=23.10 — its AMI name path uses the plain "ssd" volume type, not
// "ssd-gp3" (which would match nothing and is why this returned zero images).
const UBUNTU_2204_NAME_FILTER: &str = "ubuntu/images/hvm-ssd/ubuntu-jammy-22.04-amd64-server-*";

/// Resolves the current Ubuntu 22.04 AMI id (and its root device name, needed
/// to size the root volume at launch) for the client's region via
/// `DescribeImages` against Canonical's account, rather than hardcoding an
/// AMI id that would go stale or not exist in every region.
///
/// Deliberately not `ssm:GetParameter` (the more common convenience lookup
/// for this) — some AWS Organizations attach a Service Control Policy that
/// explicitly denies SSM Parameter Store reads regardless of the caller's
/// own IAM permissions, which made this unusable for at least one real
/// account. `DescribeImages` reaches the same "always current, never
/// hardcoded" goal through a different, commonly-unrestricted API.
pub async fn resolve_ubuntu_ami(ec2: &Client) -> Result<(String, String), String> {
    let output = ec2
        .describe_images()
        .owners(CANONICAL_OWNER_ID)
        .filters(
            Filter::builder()
                .name("name")
                .values(UBUNTU_2204_NAME_FILTER)
                .build(),
        )
        .filters(Filter::builder().name("state").values("available").build())
        .filters(
            Filter::builder()
                .name("architecture")
                .values("x86_64")
                .build(),
        )
        .filters(
            Filter::builder()
                .name("virtualization-type")
                .values("hvm")
                .build(),
        )
        .send()
        .await
        .map_err(|e| format!("Could not resolve Ubuntu AMI: {e:?}"))?;

    let image = output
        .images()
        .iter()
        .max_by_key(|i| i.creation_date().unwrap_or("").to_string())
        .ok_or_else(|| "No matching Ubuntu 22.04 AMI found via DescribeImages".to_string())?;

    let image_id = image
        .image_id()
        .ok_or_else(|| "Resolved Ubuntu AMI has no image id".to_string())?
        .to_string();

    // Canonical's HVM images have always used /dev/sda1, but read it off the
    // image itself rather than hardcoding it, in case that ever changes.
    let root_device_name = image.root_device_name().unwrap_or("/dev/sda1").to_string();

    Ok((image_id, root_device_name))
}
