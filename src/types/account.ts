export type CloudProvider = "aws";

// Mirrors the Rust `CloudAccount` struct — never carries the secret key.
export interface CloudAccount {
  id: string;
  label: string;
  provider: CloudProvider;
  region: string;
  accessKeyId: string;
  createdAt: string;
}

export interface AddCloudAccountRequest {
  label: string;
  provider: CloudProvider;
  region: string;
  accessKeyId: string;
  secretAccessKey: string;
}
