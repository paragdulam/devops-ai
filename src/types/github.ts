// Mirrors the Rust `GithubAccount`/`GithubRepo`/`LinkStatus` shapes — never
// carries the OAuth token.
export interface GithubAccount {
  id: string;
  login: string;
  avatarUrl: string | null;
  createdAt: string;
}

export interface GithubRepo {
  id: number;
  name: string;
  fullName: string;
  private: boolean;
  cloneUrl: string;
  defaultBranch: string;
}

export interface GithubLinkStart {
  linkId: string;
  userCode: string;
  verificationUri: string;
}

export type GithubLinkStatus =
  | { state: "pending" }
  | { state: "linked"; account: GithubAccount }
  | { state: "failed"; error: string };
