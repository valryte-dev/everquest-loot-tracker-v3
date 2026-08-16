import { describe, expect, it } from "vitest";
import { FEATURES } from "./features";

describe("feature catalog", () => {
  it("uses stable unique route keys", () => {
    const keys = FEATURES.map((feature) => feature.key);
    expect(new Set(keys).size).toBe(keys.length);
  });

  it("contains every required product workspace", () => {
    expect(FEATURES.map((feature) => feature.key)).toEqual([
      "live",
      "splits",
      "compounds",
      "characters",
      "imports",
      "wts",
      "items",
      "system",
      "logs",
      "help",
      "changes",
    ]);
  });

  it("assigns every workspace to a delivery slice", () => {
    expect(FEATURES.every((feature) => feature.phase >= 1 && feature.phase <= 4)).toBe(true);
  });
});
