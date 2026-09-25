import type {InventoryItem,WardrobeCatalogItem} from "../../shared/contracts";
import {getWardrobeCatalogItem} from "../../shared/backend";
import {equipmentSlotKey,type EquipmentSlotKey} from "../characters/EquipmentPaperDoll";
import type {CharacterModelProfile} from "../characters/characterProfile";
import type {WardrobeDraft} from "./model";

export const WARDROBE_STORAGE_KEY="eq-loot-tracker:wardrobe:v1";

type CatalogLookup=(itemId:number|undefined,itemName:string)=>Promise<WardrobeCatalogItem|null>;

export interface CharacterWardrobeTransfer {
 draft:WardrobeDraft;
 copied:number;
 missing:string[];
}

export async function createCharacterWardrobeTransfer(
 profile:CharacterModelProfile,
 equipped:InventoryItem[],
 lookup:CatalogLookup=getWardrobeCatalogItem,
):Promise<CharacterWardrobeTransfer>{
 const occurrences=new Map<string,number>();
 const mapped:Array<{slot:EquipmentSlotKey;item:InventoryItem}>=[];
 for(const item of equipped){
  const normalized=item.location.toLocaleLowerCase().replace(/[^a-z0-9]/g,"");
  const occurrence=occurrences.get(normalized)||0;
  occurrences.set(normalized,occurrence+1);
  const slot=equipmentSlotKey(item.location,occurrence);
  if(slot&&item.itemName.trim())mapped.push({slot,item});
 }
 const cache=new Map<string,Promise<WardrobeCatalogItem|null>>();
 const resolve=(item:InventoryItem)=>{
  const key=item.itemId?`id:${item.itemId}`:`name:${item.itemName.trim().toLocaleLowerCase()}`;
  let pending=cache.get(key);
  if(!pending){pending=lookup(item.itemId,item.itemName);cache.set(key,pending)}
  return pending;
 };
 const resolved=await Promise.all(mapped.map(async entry=>({...entry,catalogItem:await resolve(entry.item)})));
 const items:WardrobeDraft["items"]={};
 const missing:string[]=[];
 for(const entry of resolved){
  if(entry.catalogItem)items[entry.slot]=entry.catalogItem;
  else missing.push(entry.item.itemName);
 }
 return{draft:{profile:{...profile},items},copied:Object.keys(items).length,missing};
}

export function saveCharacterWardrobeTransfer(transfer:CharacterWardrobeTransfer){
 localStorage.setItem(WARDROBE_STORAGE_KEY,JSON.stringify(transfer.draft));
}