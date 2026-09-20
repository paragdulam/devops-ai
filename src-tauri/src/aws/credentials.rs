use aws_config::{BehaviorVersion, Region};
use aws_sdk_ec2::config::Credentials;

/// Builds an SDK config with an explicit static credentials provider from the
/// caller-supplied key pair. The default credential chain (env vars,
/// `~/.aws/*`) is never consulted — every credential comes from the app's own
/// account vault.
pub async fn build_sdk_config(
    region: &str,
    access_key_id: &str,
    secret_access_key: &str,
) -> aws_config::SdkConfig {
    let credentials = Credentials::new(
        access_key_id,
        secret_access_key,
        None,
        None,
        "remote-dev-machine-vault",
    );

    aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(region.to_string()))
        .credentials_provider(credentials)
        .load()
        .await
}
