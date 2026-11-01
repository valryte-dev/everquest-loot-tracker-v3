import type {InventoryItem} from "../../shared/contracts";

export interface RecipeReadyToon{character:string;craftableCopies:number}
export interface RecipeReadinessResult{missing:string[];craftableCopies:number;completeToons:RecipeReadyToon[]}

const normalized=(value:string)=>value.trim().toLocaleLowerCase().replace(/[\x60\u2018\u2019]/g,"'").replace(/\s+/g," ");

export function buildRecipeReadiness(parts:string[],inventory:InventoryItem[]):RecipeReadinessResult{
 const requirements=new Map<string,{itemName:string;quantity:number}>();
 for(const itemName of parts){
  const key=normalized(itemName),current=requirements.get(key);
  requirements.set(key,{itemName:current?.itemName||itemName,quantity:(current?.quantity||0)+1});
 }
 const rosterCounts=new Map<string,number>(),toonCounts=new Map<string,Map<string,number>>();
 for(const item of inventory){
  const key=normalized(item.itemName);
  rosterCounts.set(key,(rosterCounts.get(key)||0)+item.count);
  const counts=toonCounts.get(item.character)||new Map<string,number>();
  counts.set(key,(counts.get(key)||0)+item.count);
  toonCounts.set(item.character,counts);
 }
 const required=[...requirements.entries()];
 const missing=required.filter(([key,value])=>(rosterCounts.get(key)||0)<value.quantity).map(([,value])=>value.itemName);
 const craftableCopies=required.length?Math.min(...required.map(([key,value])=>Math.floor((rosterCounts.get(key)||0)/value.quantity))):0;
 const completeToons=[...toonCounts].map(([character,counts])=>({
  character,
  craftableCopies:required.length?Math.min(...required.map(([key,value])=>Math.floor((counts.get(key)||0)/value.quantity))):0,
 })).filter(toon=>toon.craftableCopies>0).sort((a,b)=>b.craftableCopies-a.craftableCopies||a.character.localeCompare(b.character));
 return{missing,craftableCopies,completeToons};
}
