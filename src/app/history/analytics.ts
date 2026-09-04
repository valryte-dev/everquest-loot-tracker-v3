export interface HistoryChartDatum {
  label: string;
  happenedAt: string;
  valuePp?: number;
  actor?: string;
  character?: string;
  level?: number;
  direction?: "gained" | "lost";
}

export interface RankedCategory {
  label: string;
  count: number;
}

export interface TimeBucket {
  key: string;
  label: string;
  count: number;
}

export interface RankedValue {
  label: string;
  count: number;
  valuePp: number;
}

export interface HeatCell {
  day: number;
  hour: number;
  count: number;
}

export type CardType = "Thrones" | "Crowns" | "Knights" | "Squires";
export type CardColor = "Black" | "Blue" | "Red" | "White";
export interface CardLootGroup {
  type: CardType;
  total: number;
  colors: Record<CardColor, number>;
}

export type TimeGrain = "auto" | "day" | "week" | "month" | "year";
type BucketMode = Exclude<TimeGrain, "auto"> | "hour";

const HOUR = 60 * 60 * 1000;
const DAY = 24 * HOUR;
const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

export function rankCategories(rows: HistoryChartDatum[], limit = 10): RankedCategory[] {
  const counts = new Map<string, RankedCategory>();
  for (const row of rows) {
    const label = row.label.trim() || "Unknown";
    const key = label.toLocaleLowerCase();
    const current = counts.get(key);
    if (current) current.count += 1;
    else counts.set(key, {label, count: 1});
  }
  const ranked = [...counts.values()].sort((left, right) => right.count - left.count || left.label.localeCompare(right.label));
  if (ranked.length <= limit) return ranked;
  return [...ranked.slice(0, limit), {label: "All others", count: ranked.slice(limit).reduce((sum, item) => sum + item.count, 0)}];
}

export function rankValues(rows: HistoryChartDatum[], limit = 10): RankedValue[] {
  const values = new Map<string, RankedValue>();
  for (const row of rows) {
    const valuePp = Number.isFinite(row.valuePp) ? Math.max(0, row.valuePp || 0) : 0;
    if (!valuePp) continue;
    const label = row.label.trim() || "Unknown";
    const key = label.toLocaleLowerCase();
    const current = values.get(key);
    if (current) {
      current.count += 1;
      current.valuePp = Math.max(current.valuePp, valuePp);
    } else values.set(key, {label, count: 1, valuePp});
  }
  return [...values.values()]
    .sort((left, right) => right.valuePp - left.valuePp || left.label.localeCompare(right.label))
    .slice(0, limit);
}

export function cardLootBreakdown(rows: HistoryChartDatum[]): CardLootGroup[] {
  const types:CardType[] = ["Thrones", "Crowns", "Knights", "Squires"];
  const colors:CardColor[] = ["Black", "Blue", "Red", "White"];
  const groups = new Map<CardType, CardLootGroup>(types.map(type => [type, {
    type,
    total: 0,
    colors: {Black:0, Blue:0, Red:0, White:0},
  }]));
  for (const row of rows) {
    const match = /^(?:a\s+)?(black|blue|red|white)\s+(throne|crown|knight|squire)s?$/i.exec(row.label.trim());
    if (!match) continue;
    const color = colors.find(value => value.toLowerCase() === match[1].toLowerCase());
    const type = types.find(value => value.slice(0,-1).toLowerCase() === match[2].toLowerCase());
    if (!color || !type) continue;
    const group = groups.get(type)!;
    group.colors[color] += 1;
    group.total += 1;
  }
  return types.map(type => groups.get(type)!);
}

function automaticMode(minimum: number, maximum: number): {mode: BucketMode; step: number} {
  const span = Math.max(0, maximum - minimum);
  if (span <= 2 * DAY) return {mode: "hour", step: Math.max(1, Math.ceil(span / HOUR / 24))};
  if (span <= 120 * DAY) return {mode: "day", step: Math.max(1, Math.ceil(span / DAY / 24))};
  if (span <= 730 * DAY) return {mode: "week", step: Math.max(1, Math.ceil(span / (7 * DAY) / 24))};
  const start = new Date(minimum);
  const end = new Date(maximum);
  const months = (end.getUTCFullYear() - start.getUTCFullYear()) * 12 + end.getUTCMonth() - start.getUTCMonth();
  return {mode: "month", step: Math.max(1, Math.ceil(Math.max(1, months) / 24))};
}

function bucketMode(minimum: number, maximum: number, grain: TimeGrain): {mode: BucketMode; step: number} {
  return grain === "auto" ? automaticMode(minimum, maximum) : {mode: grain, step: 1};
}

function bucketStart(value: number, mode: BucketMode, step: number): number {
  const date = new Date(value);
  if (mode === "hour") return Math.floor(value / (HOUR * step)) * HOUR * step;
  if (mode === "day") return Math.floor(value / (DAY * step)) * DAY * step;
  if (mode === "week") {
    const mondayEpoch = Date.UTC(1970, 0, 5);
    return mondayEpoch + Math.floor((value - mondayEpoch) / (7 * DAY * step)) * 7 * DAY * step;
  }
  if (mode === "year") {
    const year = Math.floor(date.getUTCFullYear() / step) * step;
    return Date.UTC(year, 0, 1);
  }
  const monthIndex = date.getUTCFullYear() * 12 + date.getUTCMonth();
  const aligned = Math.floor(monthIndex / step) * step;
  return Date.UTC(Math.floor(aligned / 12), aligned % 12, 1);
}

function nextBucket(value: number, mode: BucketMode, step: number): number {
  if (mode === "hour") return value + HOUR * step;
  if (mode === "day") return value + DAY * step;
  if (mode === "week") return value + 7 * DAY * step;
  const date = new Date(value);
  if (mode === "year") return Date.UTC(date.getUTCFullYear() + step, 0, 1);
  return Date.UTC(date.getUTCFullYear(), date.getUTCMonth() + step, 1);
}

function bucketLabel(value: number, mode: BucketMode, includeYear: boolean): string {
  const date = new Date(value);
  const day = `${MONTHS[date.getUTCMonth()]} ${date.getUTCDate()}`;
  if (mode === "hour") return `${day} ${String(date.getUTCHours()).padStart(2, "0")}:00`;
  if (mode === "month") return `${MONTHS[date.getUTCMonth()]} ${date.getUTCFullYear()}`;
  if (mode === "year") return String(date.getUTCFullYear());
  return includeYear ? `${day}, ${date.getUTCFullYear()}` : day;
}

export function bucketEvents(rows: HistoryChartDatum[], grain: TimeGrain = "auto"): TimeBucket[] {
  const timestamps = rows.map(row => Date.parse(row.happenedAt)).filter(Number.isFinite).sort((left, right) => left - right);
  if (!timestamps.length) return [];
  const minimum = timestamps[0];
  const maximum = timestamps[timestamps.length - 1];
  const {mode, step} = bucketMode(minimum, maximum, grain);
  const first = bucketStart(minimum, mode, step);
  const last = bucketStart(maximum, mode, step);
  const counts = new Map<number, number>();
  for (const timestamp of timestamps) {
    const start = bucketStart(timestamp, mode, step);
    counts.set(start, (counts.get(start) || 0) + 1);
  }
  const includeYear = new Date(minimum).getUTCFullYear() !== new Date(maximum).getUTCFullYear();
  const result: TimeBucket[] = [];
  for (let cursor = first; cursor <= last; cursor = nextBucket(cursor, mode, step)) {
    result.push({key: String(cursor), label: bucketLabel(cursor, mode, includeYear), count: counts.get(cursor) || 0});
  }
  return result;
}

export function activityHeatmap(rows: HistoryChartDatum[]): HeatCell[] {
  const counts = new Map<string, number>();
  for (const row of rows) {
    const date = new Date(row.happenedAt);
    if (!Number.isFinite(date.valueOf())) continue;
    const day = (date.getDay() + 6) % 7;
    const hour = date.getHours();
    const key = `${day}-${hour}`;
    counts.set(key, (counts.get(key) || 0) + 1);
  }
  return Array.from({length: 7 * 24}, (_, index) => {
    const day = Math.floor(index / 24);
    const hour = index % 24;
    return {day, hour, count: counts.get(`${day}-${hour}`) || 0};
  });
}
