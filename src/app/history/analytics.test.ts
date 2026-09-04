import {describe, expect, it} from "vitest";
import {activityHeatmap, bucketEvents, cardLootBreakdown, rankCategories, rankValues, type HistoryChartDatum} from "./analytics";

const datum = (label:string, happenedAt:string):HistoryChartDatum => ({label, happenedAt});

describe("history analytics", () => {
  it("keeps the ten most frequent categories and combines the remainder", () => {
    const rows:HistoryChartDatum[] = [];
    for (let index=0; index<12; index+=1) {
      for (let count=0; count<12-index; count+=1) rows.push(datum(`Item ${index}`, "2026-07-01T00:00:00Z"));
    }
    const ranked = rankCategories(rows);
    expect(ranked).toHaveLength(11);
    expect(ranked[0]).toEqual({label:"Item 0", count:12});
    expect(ranked[10]).toEqual({label:"All others", count:3});
    expect(ranked.reduce((sum, item) => sum + item.count, 0)).toBe(rows.length);
  });

  it("groups labels case-insensitively", () => {
    expect(rankCategories([
      datum("A Blue Throne", "2026-07-01T00:00:00Z"),
      datum("a blue throne", "2026-07-01T01:00:00Z"),
    ])).toEqual([{label:"A Blue Throne", count:2}]);
  });

  it("creates bounded time buckets while retaining empty periods", () => {
    const buckets = bucketEvents([
      datum("one", "2026-07-01T00:10:00Z"),
      datum("two", "2026-07-01T02:10:00Z"),
    ]);
    expect(buckets.map(bucket => bucket.count)).toEqual([1, 0, 1]);
    expect(buckets.reduce((sum, bucket) => sum + bucket.count, 0)).toBe(2);
    expect(buckets.length).toBeLessThanOrEqual(25);
  });

  it("ranks unique loot by individual estimated value", () => {
    expect(rankValues([
      {...datum("Common item", "2026-07-01T00:00:00Z"), valuePp:600},
      {...datum("common ITEM", "2026-07-01T01:00:00Z"), valuePp:600},
      {...datum("Rare item", "2026-07-01T02:00:00Z"), valuePp:1000},
      {...datum("Unpriced", "2026-07-01T03:00:00Z"), valuePp:0},
    ])).toEqual([
      {label:"Rare item", count:1, valuePp:1000},
      {label:"Common item", count:2, valuePp:600},
    ]);
  });

  it("supports explicit day, week, month, and year timeline views", () => {
    const rows = [
      datum("one", "2025-12-31T12:00:00Z"),
      datum("two", "2026-01-02T12:00:00Z"),
    ];
    expect(bucketEvents(rows, "day").reduce((sum, bucket) => sum + bucket.count, 0)).toBe(2);
    expect(bucketEvents(rows, "week").reduce((sum, bucket) => sum + bucket.count, 0)).toBe(2);
    expect(bucketEvents(rows, "month").map(bucket => bucket.label)).toEqual(["Dec 2025", "Jan 2026"]);
    expect(bucketEvents(rows, "year").map(bucket => bucket.label)).toEqual(["2025", "2026"]);
  });

  it("builds a complete weekly hour-of-day activity fingerprint", () => {
    const cells = activityHeatmap([datum("one", "2026-08-31T12:00:00")]);
    expect(cells).toHaveLength(168);
    expect(cells.reduce((sum, cell) => sum + cell.count, 0)).toBe(1);
  });

  it("groups special card loot by type and color", () => {
    const groups = cardLootBreakdown([
      datum("A Blue Throne", "2026-07-01T00:00:00Z"),
      datum("Blue Throne", "2026-07-01T01:00:00Z"),
      datum("A Red Knight", "2026-07-01T02:00:00Z"),
      datum("Not a card", "2026-07-01T03:00:00Z"),
    ]);
    expect(groups.find(group => group.type === "Thrones")).toMatchObject({total:2, colors:{Black:0, Blue:2, Red:0, White:0}});
    expect(groups.find(group => group.type === "Knights")).toMatchObject({total:1, colors:{Black:0, Blue:0, Red:1, White:0}});
    expect(groups.reduce((sum, group) => sum + group.total, 0)).toBe(3);
  });

  it("ignores invalid timestamps", () => {
    expect(bucketEvents([datum("bad", "not-a-date")])).toEqual([]);
  });
});
