import { describe, expect, it } from "vitest";
import { formatElapsed } from "./time";

describe("formatElapsed", () => {
  it("formats zero and sub-minute durations as seconds", () => {
    expect(formatElapsed(0)).toBe("0s");
    expect(formatElapsed(45_000)).toBe("45s");
  });

  it("formats sub-hour durations as minutes and seconds", () => {
    expect(formatElapsed(23 * 60_000 + 14_000)).toBe("23m 14s");
  });

  it("formats durations over an hour as hours, minutes, and seconds", () => {
    expect(formatElapsed(2 * 3_600_000 + 5 * 60_000 + 9_000)).toBe("2h 5m 9s");
  });
});
