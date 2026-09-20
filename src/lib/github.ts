import { invoke } from "@tauri-apps/api/core";
import type {
  GithubAccount,
  GithubLinkStart,
  GithubLinkStatus,
  GithubRepo,
} from "../types/github";

export async function linkGithubAccount(): Promise<GithubLinkStart> {
  return invoke<GithubLinkStart>("link_github_account");
}

export async function getGithubLinkStatus(linkId: string): Promise<GithubLinkStatus> {
  return invoke<GithubLinkStatus>("get_github_link_status", { linkId });
}

export async function listGithubAccounts(): Promise<GithubAccount[]> {
  return invoke<GithubAccount[]>("list_github_accounts");
}

export async function deleteGithubAccount(id: string): Promise<void> {
  await invoke("delete_github_account", { id });
}

export async function listGithubRepos(accountId: string): Promise<GithubRepo[]> {
  return invoke<GithubRepo[]>("list_github_repos", { accountId });
}
