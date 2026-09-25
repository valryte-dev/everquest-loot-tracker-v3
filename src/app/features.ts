export type FeatureKey = "live" | "linked" | "tracked" | "activity-history" | "death-reports" | "damage" | "dps-calculator" | "dot-lab" | "ch-lab" | "splits" | "compounds" | "merchant" | "wts" | "characters" | "wardrobe" | "spells" | "spell-research" | "gems" | "quest-items" | "imports" | "items" | "database" | "system" | "logs" | "help" | "changes";
export type FeatureGroup = "Loot" | "Combat" | "Trading" | "Roster" | "Data" | "Application";
export interface FeatureDefinition { key:FeatureKey; label:string; shortLabel:string; description:string; icon:string; phase:1|2|3|4; group:FeatureGroup }
export const FEATURE_GROUPS:FeatureGroup[] = ["Loot", "Combat", "Trading", "Roster", "Data", "Application"];
export const FEATURES:FeatureDefinition[] = [
 {key:"live",label:"Live Loot",shortLabel:"Live Loot",icon:"◇",phase:1,group:"Loot",description:"Active log, current group, mob correlation, values, sharing and bulk cleanup."},
 {key:"linked",label:"Linked Loot",shortLabel:"Linked Loot",icon:"↗",phase:2,group:"Loot",description:"Items linked in group and guild chat with speaker, channel, time and current 30-day WTS value."},
 {key:"tracked",label:"Tracked Loot",shortLabel:"Tracked Loot",icon:"◎",phase:2,group:"Loot",description:"Important loot snapshots retained independently from the temporary live-loot list."},
 {key:"activity-history",label:"Log History",shortLabel:"History",icon:"◴",phase:2,group:"Loot",description:"Permanent, searchable history of loot, slain mobs and items offered across every character log."},
 {key:"death-reports",label:"Death Reports",shortLabel:"Death Reports",icon:"☠",phase:2,group:"Combat",description:"Search every character log for deaths and review the killer with the preceding 30 log entries."},
 {key:"damage",label:"Damage Tracker",shortLabel:"Damage Tracker",icon:"D",phase:2,group:"Combat",description:"Track personal melee and spell damage for each mob encounter, including DPS, attack mix and damage over time."},
 {key:"dps-calculator",label:"DPS Calculator",shortLabel:"DPS Calculator",icon:"DPS",phase:3,group:"Combat",description:"Estimate P99 melee DPS and populate class, level, gear Strength, haste, and weapons from saved characters or Wardrobe."},
 {key:"dot-lab",label:"Proc & DoT Training Lab",shortLabel:"Proc/DoT Lab",icon:"T",phase:2,group:"Combat",description:"Paste combat logs into an isolated simulation and inspect every proc, DoT match, attribution, tick, and chart decision."},
 {key:"ch-lab",label:"Complete Heal Chain Lab",shortLabel:"CH Lab",icon:"CH",phase:2,group:"Combat",description:"Paste guild Complete Heal calls into an isolated production-parser preview and exercise live cadence visualization states."},
 {key:"splits",label:"Splits & Payouts",shortLabel:"Splits",icon:"÷",phase:2,group:"Loot",description:"Track held loot, reconcile pasted seller lists, manage pending payouts, aliases, holders, completed sales and consumed history."},
 {key:"compounds",label:"Compound Projects",shortLabel:"Compounds",icon:"⬡",phase:3,group:"Loot",description:"Recipes, reusable templates, contributions, readiness and estimated value."},
 {key:"merchant",label:"Merchant Watch",shortLabel:"Merchant",icon:"¤",phase:2,group:"Trading",description:"Opt-in auction monitoring for WTS offers, WTB requests, direct tells and PigParse comparisons."},
 {key:"wts",label:"Want to Sell",shortLabel:"WTS Groups",icon:"$",phase:3,group:"Trading",description:"Character-scoped sale groups and EverQuest Page 10 social export."},
 {key:"characters",label:"Characters",shortLabel:"Characters",icon:"♙",phase:2,group:"Roster",description:"Roster summary plus character equipment, carried and banked items, cards, spells and recipes."},
 {key:"spells",label:"Roster Spells",shortLabel:"Spells",icon:"✦",phase:2,group:"Roster",description:"Search spell scrolls and scribed spellbooks across every imported character."},
 {key:"spell-research",label:"Spell Research",shortLabel:"Research",icon:"R",phase:3,group:"Roster",description:"Track every P99 research recipe and the words, pages, runes, chained spell scrolls, and supporting items held across the roster."},
 {key:"gems",label:"Velious Armor Gems",shortLabel:"Armor Gems",icon:"◆",phase:2,group:"Roster",description:"Special Velious armor gems held across the entire character roster."},
 {key:"imports",label:"Import Center",shortLabel:"Import Center",icon:"⇩",phase:2,group:"Data",description:"Drop character exports, review import results, and publish selected files to P99 Planner."},
 {key:"items",label:"Master Items",shortLabel:"Master Items",icon:"▦",phase:2,group:"Data",description:"Item IDs, PigParse 30-day WTS values and protected manual corrections."},
 {key:"database",label:"Database Management",shortLabel:"Database",icon:"DB",phase:2,group:"Data",description:"Inspect database growth, storage categories, health, backups, and preview safe retention policies."},
 {key:"system",label:"System",shortLabel:"System",icon:"⚙",phase:1,group:"Application",description:"Application updates, paths, watchers, aliases, themes, migration and database backups."},
 {key:"logs",label:"Application Logs",shortLabel:"Logs",icon:"≡",phase:1,group:"Application",description:"Rolling structured diagnostics with levels, search, pause, copy and export."},
 {key:"help",label:"Help",shortLabel:"Help",icon:"?",phase:4,group:"Application",description:"Brief guides for every workflow in the application."},
 {key:"changes",label:"Change Log",shortLabel:"Change Log",icon:"↻",phase:4,group:"Application",description:"Release history and noteworthy behavior changes."},
 {key:"quest-items",label:"Quest Item Readiness",shortLabel:"Quest Items",icon:"Q",phase:3,group:"Roster",description:"Roster-wide Plane of Sky, Velious armor, and epic quest component readiness from the P99 wiki catalog."},
 {key:"wardrobe",label:"Wardrobe",shortLabel:"Wardrobe",icon:"W",phase:3,group:"Roster",description:"Build a race-, class-, and gender-aware outfit using the 3D model and a stat-filterable item catalog."},
];
