import { describe, expect, it } from "vitest";
import { mergePageSnapshot, pageSnapshotKey } from "./liveRefresh";

describe("live page refreshes", () => {
  it("uses the lightweight snapshot for live deltas", () => {
    expect(pageSnapshotKey("live", "live-delta")).toBe("live-stream");
    expect(pageSnapshotKey("live", "full")).toBe("live");
    expect(pageSnapshotKey("damage", "live-delta")).toBe("damage");
  });

  it("retains reference catalogs while replacing changing live rows", () => {
    const current = {
      settings: { logFolder: "old" },
      members: ["old member"],
      items: ["cached item"],
      mobs: ["cached mob"],
      loot: [{ id: 1, name: "old loot" }],
      tracked: ["old tracked"],
    };
    const incoming = {
      settings: { logFolder: "new" },
      members: ["new member"],
      items: [],
      mobs: [],
      loot: [{ id: 2, name: "new loot" }],
      tracked: [],
    };

    expect(mergePageSnapshot(current, incoming, "live", "live-delta")).toEqual({
      settings: { logFolder: "new" },
      members: ["new member"],
      items: ["cached item"],
      mobs: ["cached mob"],
      loot: [
        { id: 2, name: "new loot" },
        { id: 1, name: "old loot" },
      ],
      tracked: ["old tracked"],
    });
  });

  it("replaces the snapshot for a full refresh", () => {
    const current = { items: ["old"] };
    const incoming = { items: ["new"] };
    expect(mergePageSnapshot(current, incoming, "live", "full")).toBe(incoming);
  });
});
