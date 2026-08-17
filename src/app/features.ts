export type FeatureKey="live"|"merchant"|"splits"|"compounds"|"characters"|"imports"|"wts"|"items"|"system"|"logs"|"help"|"changes";
export interface FeatureDefinition{key:FeatureKey;label:string;shortLabel:string;description:string;icon:string;phase:1|2|3|4}
export const FEATURES:FeatureDefinition[]=[
 {key:"live",label:"Live Loot",shortLabel:"Loot",icon:"◇",phase:1,description:"Active log, current group, mob correlation, values, sharing and bulk cleanup."},
 {key:"merchant",label:"Merchant Watch",shortLabel:"Merchant",icon:"¤",phase:2,description:"Opt-in auction monitoring for WTS offers, WTB requests, direct tells and PigParse comparisons."},
 {key:"splits",label:"Splits & Payouts",shortLabel:"Splits",icon:"÷",phase:2,description:"Active splits, aliases, holders, proceeds and sold or consumed history."},
 {key:"compounds",label:"Compound Projects",shortLabel:"Compounds",icon:"⬡",phase:3,description:"Recipes, reusable templates, contributions, readiness and estimated value."},
 {key:"characters",label:"Characters",shortLabel:"Roster",icon:"♙",phase:2,description:"Equipment, carried and banked items, cards, spells and recipe readiness."},
 {key:"imports",label:"Import Center",shortLabel:"Import",icon:"⇩",phase:2,description:"Drop character exports, review import results, and publish selected files to P99 Planner."},
 {key:"wts",label:"Want to Sell",shortLabel:"WTS",icon:"$",phase:3,description:"Character-scoped sale groups and EverQuest Page 10 social export."},
 {key:"items",label:"Master Items",shortLabel:"Items",icon:"▦",phase:2,description:"Item IDs, PigParse 30-day WTS values and protected manual corrections."},
 {key:"system",label:"System",shortLabel:"System",icon:"⚙",phase:1,description:"Paths, watchers, planner imports, aliases, themes, migration and backups."},
 {key:"logs",label:"Application Logs",shortLabel:"Logs",icon:"≡",phase:1,description:"Rolling structured diagnostics with levels, search, pause, copy and export."},
 {key:"help",label:"Help",shortLabel:"Help",icon:"?",phase:4,description:"Brief guides for every workflow in the application."},
 {key:"changes",label:"Change Log",shortLabel:"Changes",icon:"↻",phase:4,description:"Release history and noteworthy behavior changes."},
];
