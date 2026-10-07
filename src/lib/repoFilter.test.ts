import { describe, expect, it } from "vitest";
import { matchesQuery } from "./repoFilter";

describe("matchesQuery", () => {
  it("matches everything for a blank query", () => {
    expect(matchesQuery("", "acme/api")).toBe(true);
    expect(matchesQuery("   ", "acme/api")).toBe(true);
  });

  it("matches case-insensitively on any field", () => {
    expect(matchesQuery("API", "acme/api")).toBe(true);
    expect(matchesQuery("web", "acme/api", "web-frontend")).toBe(true);
  });

  it("rejects when no field matches", () => {
    expect(matchesQuery("zzz", "acme/api")).toBe(false);
  });
});
