use aws_sdk_ec2::Client;
use russh::keys::ssh_key::{Algorithm, LineEnding, PrivateKey};

/// Generates a fresh Ed25519 keypair in-process (no `ssh-keygen` subprocess).
/// Returns `(public_key_openssh_line, private_key_openssh_pem)`.
pub fn generate_keypair() -> Result<(String, String), String> {
    let private_key = PrivateKey::random(&mut rand::rngs::ThreadRng::default(), Algorithm::Ed25519)
        .map_err(|e| format!("Could not generate SSH keypair: {e}"))?;

    let public_key = private_key
        .public_key()
        .to_openssh()
        .map_err(|e| format!("Could not encode SSH public key: {e}"))?;

    let private_pem = private_key
        .to_openssh(LineEnding::LF)
        .map_err(|e| format!("Could not encode SSH private key: {e}"))?
        .to_string();

    Ok((public_key, private_pem))
}

/// Imports a public key as an EC2 key pair so `RunInstances` can grant SSH
/// access without ever uploading the private half anywhere.
pub async fn import_key_pair(
    ec2: &Client,
    rental_id: &str,
    public_key: &str,
) -> Result<String, String> {
    let key_name = format!("rdm-{rental_id}");
    ec2.import_key_pair()
        .key_name(&key_name)
        .public_key_material(aws_sdk_ec2::primitives::Blob::new(public_key.as_bytes()))
        .send()
        .await
        .map_err(|e| format!("Could not import EC2 key pair: {e:?}"))?;
    Ok(key_name)
}

/// Best-effort delete, mirroring `aws::network::delete_security_group`'s
/// log-and-continue style — used identically on Stop and on a failed
/// provisioning attempt so nothing billed or exposed is ever left behind.
pub async fn delete_key_pair(ec2: &Client, key_name: &str) -> Result<(), String> {
    ec2.delete_key_pair()
        .key_name(key_name)
        .send()
        .await
        .map_err(|e| format!("Could not delete EC2 key pair {key_name}: {e:?}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Diagnostic for "SSH authentication was rejected": confirms the private
    /// PEM this module hands back actually round-trips (via the same
    /// `decode_secret_key` the SSH client uses) to the identical public key
    /// that gets imported to AWS, entirely offline.
    #[test]
    fn generated_private_key_round_trips_to_the_same_public_key() {
        let (public_key, private_pem) = generate_keypair().unwrap();
        let decoded = russh::keys::decode_secret_key(&private_pem, None).unwrap();
        let round_tripped_public = decoded.public_key().to_openssh().unwrap();

        // Compare only the algorithm+key material fields (first two
        // whitespace-separated tokens), since OpenSSH pubkey lines can carry
        // a trailing comment that isn't part of the key itself.
        let key_fields =
            |line: &str| -> String { line.split_whitespace().take(2).collect::<Vec<_>>().join(" ") };
        assert_eq!(
            key_fields(&public_key),
            key_fields(&round_tripped_public),
            "public_key={public_key:?} round_tripped={round_tripped_public:?}"
        );
    }
}
