import { invoke } from "@tauri-apps/api/core";
import type { AppSnapshot, BootstrapStatus } from "./contracts";

const previewStatus:BootstrapStatus={appVersion:"3.2.0",platform:"browser-preview",databasePath:"Desktop app required",databaseReady:false,schemaVersion:0,legacyDatabase:false};
const previewSnapshot:AppSnapshot={settings:{theme:"midnight",merchant_mode_enabled:"false"},members:[],loot:[],splits:[],tracked:[],linkedLoot:[],history:[],items:[],inventory:[],spells:[],wts:[],aliases:[],mobs:[],logs:[],imports:[],merchant:[],compound:{projects:[],templates:[],activeId:null}};
const desktop=()=>"__TAURI_INTERNALS__" in window;
export const bootstrapStatus=()=>desktop()?invoke<BootstrapStatus>("bootstrap_status"):Promise.resolve(previewStatus);
export const getSnapshot=()=>desktop()?invoke<AppSnapshot>("app_snapshot"):Promise.resolve(previewSnapshot);
export const mutate=async(action:string,payload:Record<string,unknown>={})=>desktop()?invoke<unknown>("mutate_app",{request:{action,payload}}):null;
