import { invoke } from "@tauri-apps/api/core";
import type { ActivityHistorySnapshot, AppSnapshot, BootstrapStatus, SpellCatalogStatus, SpellInfo } from "./contracts";

const previewStatus:BootstrapStatus={appVersion:"3.2.0",platform:"browser-preview",databasePath:"Desktop app required",databaseReady:false,schemaVersion:0,legacyDatabase:false};
const previewSnapshot:AppSnapshot={settings:{theme:"midnight",merchant_mode_enabled:"false"},members:[],loot:[],splits:[],tracked:[],linkedLoot:[],history:[],items:[],inventory:[],spells:[],wts:[],aliases:[],mobs:[],logs:[],imports:[],merchant:[],compound:{projects:[],templates:[],activeId:null}};
const desktop=()=>"__TAURI_INTERNALS__" in window;
export const bootstrapStatus=()=>desktop()?invoke<BootstrapStatus>("bootstrap_status"):Promise.resolve(previewStatus);
export const getSnapshot=()=>desktop()?invoke<AppSnapshot>("app_snapshot"):Promise.resolve(previewSnapshot);
export const getPageSnapshot=(page:string)=>desktop()?invoke<AppSnapshot>("app_page_snapshot",{page}):Promise.resolve(previewSnapshot);
export const getActivityHistorySnapshot=()=>desktop()?invoke<ActivityHistorySnapshot>("activity_history_snapshot"):Promise.resolve({loot:[],mobs:[],offers:[],levels:[]});
export const getRevision=()=>desktop()?invoke<number>("app_revision"):Promise.resolve(0);
export const mutate=async(action:string,payload:Record<string,unknown>={})=>desktop()?invoke<unknown>("mutate_app",{request:{action,payload}}):null;
export const getSpellInfo=(spellName:string)=>desktop()?invoke<SpellInfo>("spell_info",{spellName}):Promise.reject(new Error("Spell information is available in the desktop app."));
const previewSpellCatalog:SpellCatalogStatus={cachedCount:0,processed:0,saved:0,failed:0,refreshing:false};
export const getSpellCatalogStatus=()=>desktop()?invoke<SpellCatalogStatus>("spell_catalog_status"):Promise.resolve(previewSpellCatalog);
export const reloadSpellCatalog=()=>desktop()?invoke<SpellCatalogStatus>("reload_spell_catalog"):Promise.resolve(previewSpellCatalog);
