import { describe, expect, it } from "vitest";
import appSource from "./App.tsx?raw";

describe("UI source encoding", () => {
  it("contains no common UTF-8 mojibake markers", () => {
    expect(appSource).not.toMatch(/[ÂÃ]|â/);
  });
});