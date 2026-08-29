export interface BootstrapStatus { appVersion:string; platform:string; databasePath:string; databaseReady:boolean; schemaVersion:number; legacyDatabase:boolean }
export interface Member { id:number; name:string; active:boolean }
export interface Loot { id:number; happenedAt:string; itemName:string; mobName?:string; looterName?:string; valuePp?:number; splitListed:boolean; attendees:string[] }
export interface Split { key:string; itemName:string; addedAt:string; mobName?:string; looterName?:string; payoutValuePp?:number; marketValuePp?:number; attendees:string[] }
export interface TrackedLoot { id:number; sourceLootId?:number; happenedAt:string; trackedAt:string; itemName:string; mobName?:string; looterName?:string; valuePp?:number; attendees:string[] }
export interface LinkedLoot { id:number; happenedAt:string; channel:"group"|"guild"; speakerName:string; itemName:string; itemId?:number; valuePp?:number; valueBasis?:string; count30d:number }
export interface History { id:number; itemName:string; mobName?:string; looterName?:string; valuePp:number; disposition:string; note:string; completedAt:string; attendees:string[] }
export interface MasterItem { id:number; name:string; valuePp:number; count30d:number; lastSeen:string; manual:boolean; source:string }
export interface InventoryItem { character:string; importedAt:string; id:number; location:string; itemName:string; itemId?:number; count:number; slots?:number; valuePp?:number }
export interface Spell { character:string; importedAt:string; slot?:number; spellName:string }
export interface SpellClassInfo { name:string; level:number }
export interface SpellEffectInfo { slot:number; description:string }
export interface SpellInfo { spellName:string; wikiUrl:string; description:string; classes:SpellClassInfo[]; effects:SpellEffectInfo[]; mana:string; skill:string; castingTime:string; recastTime:string; fizzleTime:string; resist:string; range:string; targetType:string; spellType:string; duration:string; reagent:string; focus:string; whereToObtain:string; fetchedAt:string; stale:boolean }
export interface SpellCatalogStatus { cachedCount:number; processed:number; saved:number; failed:number; refreshing:boolean; startedAt?:string; lastRefreshAt?:string; lastError?:string }
export interface WtsGroup { id:number; character:string; name:string; createdAt:string; updatedAt:string; items:string[]; itemIds:(number|null)[] }
export interface Alias { alias:string; canonical:string }
export interface AppLog { id:number; happenedAt:string; level:string; area:string; message:string }
export interface ImportRecord { id:number; happenedAt:string; fileName:string; status:string; reviewUrl?:string; detail?:string }
export interface MerchantListingItem { id:number; itemName:string; itemId?:number; askingPricePp?:number; marketValuePp?:number; marketCount30d:number }
export interface MerchantMessage { id:number; happenedAt:string; kind:"wts"|"wtb"|"tell"; speakerName:string; message:string; items:MerchantListingItem[] }
export interface CompoundWorkspace { projects:any[]; templates:any[]; activeId?:string|null }
export interface AppSnapshot { settings:Record<string,string>; members:Member[]; loot:Loot[]; splits:Split[]; tracked:TrackedLoot[]; linkedLoot:LinkedLoot[]; history:History[]; items:MasterItem[]; inventory:InventoryItem[]; spells:Spell[]; wts:WtsGroup[]; aliases:Alias[]; mobs:string[]; logs:AppLog[]; imports:ImportRecord[]; merchant:MerchantMessage[]; compound:CompoundWorkspace }
export type LoadingState<T>={kind:"loading"}|{kind:"ready";value:T}|{kind:"error";message:string};
