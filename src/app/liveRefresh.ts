export type PageRefreshMode = "full" | "live-delta";

function mergeRecentRows(current: unknown, incoming: unknown, limit: number): unknown {
  if (!Array.isArray(current) || !Array.isArray(incoming)) return incoming;
  const rows = [...incoming, ...current];
  const seen = new Set<unknown>();
  return rows.filter((row) => {
    const id = row && typeof row === "object" ? (row as { id?: unknown }).id : row;
    if (seen.has(id)) return false;
    seen.add(id);
    return true;
  }).slice(0, limit);
}

export function pageSnapshotKey(page: string, mode: PageRefreshMode): string {
  return page === "live" && mode === "live-delta" ? "live-stream" : page;
}

export function mergePageSnapshot<T extends object>(
  current: T,
  incoming: T,
  page: string,
  mode: PageRefreshMode,
): T {
  if (page !== "live" || mode !== "live-delta") return incoming;

  const updates = incoming as unknown as Record<string, unknown>;

  return {
    ...current,
    settings: updates.settings,
    members: updates.members,
    loot: mergeRecentRows(
      (current as unknown as Record<string, unknown>).loot,
      updates.loot,
      1000,
    ),
  } as T;
}
