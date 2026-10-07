import { invoke } from "@tauri-apps/api/core";
import type {
  GithubAccount,
  GithubLinkStart,
  GithubLinkStatus,
  GithubRepo,
} from "../types/github";
import type { ProjectDetection } from "../types/project";

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

export async function detectGithubProject(
  accountId: string,
  fullName: string,
  branch: string,
): Promise<ProjectDetection> {
  return invoke<ProjectDetection>("detect_github_project", { accountId, fullName, branch });
}
