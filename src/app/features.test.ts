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
      "linked",
      "tracked",
      "activity-history",
      "death-reports",
      "damage",
      "dot-lab",
      "ch-lab",
      "splits",
      "compounds",
      "merchant",
      "wts",
      "characters",
      "spells",
      "gems",
      "imports",
      "items",
      "database",
      "system",
      "logs",
      "help",
      "changes",
      "quest-items",
      "wardrobe",
    ]);
  });

  it("assigns every workspace to a delivery slice", () => {
    expect(FEATURES.every((feature) => feature.phase >= 1 && feature.phase <= 4)).toBe(true);
  });
});
