export type FeatureKey =
  | "live"
  | "splits"
  | "compounds"
  | "characters"
  | "wts"
  | "items"
  | "system"
  | "logs"
  | "help";

export interface FeatureDefinition {
  key: FeatureKey;
  label: string;
  shortLabel: string;
  description: string;
  icon: string;
  phase: 1 | 2 | 3 | 4;
}

export const FEATURES: FeatureDefinition[] = [
  { key: "live", label: "Live Loot", shortLabel: "Loot", icon: "◇", phase: 1, description: "Active log, group roster, mob correlation, loot review, filtering, sorting, and bulk cleanup." },
  { key: "splits", label: "Splits & Payouts", shortLabel: "Splits", icon: "÷", phase: 2, description: "Split inventory, holders, looters, aliases, proceeds, summaries, and sold or consumed history." },
  { key: "compounds", label: "Compound Projects", shortLabel: "Compounds", icon: "⬡", phase: 3, description: "Master-linked recipes, reusable templates, contributions, ownership, completion, and direct WTS handoff." },
  { key: "characters", label: "Characters", shortLabel: "Roster", icon: "♙", phase: 2, description: "Inventory, equipment, bank, cards, spells, recipe readiness, and roster-wide searchable summaries." },
  { key: "wts", label: "Want to Sell", shortLabel: "WTS", icon: "$", phase: 3, description: "Character-scoped sale groups and safe EverQuest Page 10 social export with clickable item links." },
  { key: "items", label: "Master Items", shortLabel: "Items", icon: "▦", phase: 2, description: "Green item IDs, PigParse 30-day WTS values, protected manual corrections, filtering, and sorting." },
  { key: "system", label: "System", shortLabel: "System", icon: "⚙", phase: 1, description: "Cross-platform paths, watchers, API imports, aliases, themes, database migration, and diagnostics." },
  { key: "logs", label: "Application Logs", shortLabel: "Logs", icon: "≡", phase: 1, description: "Rolling structured logs with levels, search, pause, copy, and export." },
  { key: "help", label: "Help & Changes", shortLabel: "Help", icon: "?", phase: 4, description: "Feature help, onboarding, migration status, keyboard access, and release history." },
];
