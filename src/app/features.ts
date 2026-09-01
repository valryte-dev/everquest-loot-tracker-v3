export type FeatureKey = "live" | "linked" | "tracked" | "splits" | "compounds" | "merchant" | "wts" | "characters" | "spells" | "gems" | "imports" | "items" | "system" | "logs" | "help" | "changes";
export type FeatureGroup = "Loot" | "Trading" | "Roster" | "Data" | "Application";
export interface FeatureDefinition { key:FeatureKey; label:string; shortLabel:string; description:string; icon:string; phase:1|2|3|4; group:FeatureGroup }
export const FEATURE_GROUPS:FeatureGroup[] = ["Loot", "Trading", "Roster", "Data", "Application"];
export const FEATURES:FeatureDefinition[] = [
 {key:"live",label:"Live Loot",shortLabel:"Live Loot",icon:"◇",phase:1,group:"Loot",description:"Active log, current group, mob correlation, values, sharing and bulk cleanup."},
 {key:"linked",label:"Linked Loot",shortLabel:"Linked Loot",icon:"↗",phase:2,group:"Loot",description:"Items linked in group and guild chat with speaker, channel, time and current 30-day WTS value."},
 {key:"tracked",label:"Tracked Loot",shortLabel:"Tracked Loot",icon:"◎",phase:2,group:"Loot",description:"Important loot snapshots retained independently from the temporary live-loot list."},
 {key:"splits",label:"Splits & Payouts",shortLabel:"Splits",icon:"÷",phase:2,group:"Loot",description:"Track held loot, sold items awaiting payout, completed payouts, aliases, holders and consumed history."},
 {key:"compounds",label:"Compound Projects",shortLabel:"Compounds",icon:"⬡",phase:3,group:"Loot",description:"Recipes, reusable templates, contributions, readiness and estimated value."},
 {key:"merchant",label:"Merchant Watch",shortLabel:"Merchant",icon:"¤",phase:2,group:"Trading",description:"Opt-in auction monitoring for WTS offers, WTB requests, direct tells and PigParse comparisons."},
 {key:"wts",label:"Want to Sell",shortLabel:"WTS Groups",icon:"$",phase:3,group:"Trading",description:"Character-scoped sale groups and EverQuest Page 10 social export."},
 {key:"characters",label:"Characters",shortLabel:"Characters",icon:"♙",phase:2,group:"Roster",description:"Roster summary plus character equipment, carried and banked items, cards, spells and recipes."},
 {key:"spells",label:"Roster Spells",shortLabel:"Spells",icon:"✦",phase:2,group:"Roster",description:"Search spell scrolls and scribed spellbooks across every imported character."},
 {key:"gems",label:"Velious Armor Gems",shortLabel:"Armor Gems",icon:"◆",phase:2,group:"Roster",description:"Special Velious armor gems held across the entire character roster."},
 {key:"imports",label:"Import Center",shortLabel:"Import Center",icon:"⇩",phase:2,group:"Data",description:"Drop character exports, review import results, and publish selected files to P99 Planner."},
 {key:"items",label:"Master Items",shortLabel:"Master Items",icon:"▦",phase:2,group:"Data",description:"Item IDs, PigParse 30-day WTS values and protected manual corrections."},
 {key:"system",label:"System",shortLabel:"System",icon:"⚙",phase:1,group:"Application",description:"Paths, watchers, planner imports, aliases, themes, migration and backups."},
 {key:"logs",label:"Application Logs",shortLabel:"Logs",icon:"≡",phase:1,group:"Application",description:"Rolling structured diagnostics with levels, search, pause, copy and export."},
 {key:"help",label:"Help",shortLabel:"Help",icon:"?",phase:4,group:"Application",description:"Brief guides for every workflow in the application."},
 {key:"changes",label:"Change Log",shortLabel:"Change Log",icon:"↻",phase:4,group:"Application",description:"Release history and noteworthy behavior changes."},
];
