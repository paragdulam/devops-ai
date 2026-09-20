import { invoke } from "@tauri-apps/api/core";
import type { AddCloudAccountRequest, CloudAccount } from "../types/account";

export async function listCloudAccounts(): Promise<CloudAccount[]> {
  return invoke<CloudAccount[]>("list_cloud_accounts");
}

export async function addCloudAccount(req: AddCloudAccountRequest): Promise<CloudAccount> {
  return invoke<CloudAccount>("add_cloud_account", {
    label: req.label,
    provider: req.provider,
    region: req.region,
    accessKeyId: req.accessKeyId,
    secretAccessKey: req.secretAccessKey,
  });
}

export async function deleteCloudAccount(id: string): Promise<void> {
  await invoke("delete_cloud_account", { id });
}
