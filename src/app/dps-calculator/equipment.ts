import type {InventoryItem,WardrobeCatalogItem} from "../../shared/contracts";
import {getWardrobeCatalogItem} from "../../shared/backend";
import {equipmentSlotKey,type EquipmentSlotKey} from "../characters/EquipmentPaperDoll";
import type {WardrobeDraft} from "../wardrobe/model";
import {weaponHandedness} from "../wardrobe/model";

export interface DpsEquipmentLoadout {items:Partial<Record<EquipmentSlotKey,WardrobeCatalogItem>>;gearStrength:number;haste:number;primary?:WardrobeCatalogItem;secondary?:WardrobeCatalogItem;mainHand:"1h"|"2h";missing:string[]}
type CatalogLookup=(itemId:number|undefined,itemName:string)=>Promise<WardrobeCatalogItem|null>;

export function summarizeDpsEquipment(items:Partial<Record<EquipmentSlotKey,WardrobeCatalogItem>>,missing:string[]=[]):DpsEquipmentLoadout{
 const equipped=Object.values(items).filter((item):item is WardrobeCatalogItem=>Boolean(item));
 const primary=items.primary,secondary=items.secondary;
 return{items,gearStrength:equipped.reduce((sum,item)=>sum+item.strength,0),haste:equipped.reduce((highest,item)=>Math.max(highest,item.haste),0),primary,secondary,mainHand:primary&&weaponHandedness(primary)==="2h"?"2h":"1h",missing};
}

export async function resolveCharacterDpsEquipment(inventory:InventoryItem[],lookup:CatalogLookup=getWardrobeCatalogItem):Promise<DpsEquipmentLoadout>{
 const occurrences=new Map<string,number>(),mapped:Array<{slot:EquipmentSlotKey;item:InventoryItem}>=[];
 for(const item of inventory){
  const normalized=item.location.toLocaleLowerCase().replace(/[^a-z0-9]/g,"");
  const occurrence=occurrences.get(normalized)||0;occurrences.set(normalized,occurrence+1);
  const slot=equipmentSlotKey(item.location,occurrence);if(slot&&item.itemName.trim())mapped.push({slot,item});
 }
 const cache=new Map<string,Promise<WardrobeCatalogItem|null>>();
 const resolved=await Promise.all(mapped.map(async entry=>{
  const key=entry.item.itemId?`id:${entry.item.itemId}`:`name:${entry.item.itemName.trim().toLocaleLowerCase()}`;
  let pending=cache.get(key);if(!pending){pending=lookup(entry.item.itemId,entry.item.itemName);cache.set(key,pending)}
  return{...entry,catalogItem:await pending};
 }));
 const items:Partial<Record<EquipmentSlotKey,WardrobeCatalogItem>>={},missing:string[]=[];
 for(const entry of resolved){if(entry.catalogItem)items[entry.slot]=entry.catalogItem;else missing.push(entry.item.itemName)}
 return summarizeDpsEquipment(items,missing);
}

export function loadWardrobeDpsEquipment(storage:string|null):{draft?:WardrobeDraft;loadout?:DpsEquipmentLoadout}{
 try{const draft=JSON.parse(storage||"") as WardrobeDraft;if(!draft?.profile||!draft?.items)return{};return{draft,loadout:summarizeDpsEquipment(draft.items)}}catch{return{}}
}