import type {InventoryItem,ResearchCatalogItem,ResearchComponentKind} from "../../shared/contracts";

export interface ResearchHolding{character:string;count:number;locations:string[]}
export interface ResearchComponentView extends ResearchCatalogItem{held:number;holders:ResearchHolding[]}
export interface ResearchRecipeView{
 key:string;recipeId:number;className:ResearchCatalogItem["className"];level:number;spellName:string;
 spellItemId?:number;spellValuePp?:number;trivial:string;researchOnly:boolean;availability:string;
 sourceUrl:string;components:ResearchComponentView[];ownedRequirements:number;totalRequirements:number;
 ready:boolean;craftableCopies:number;heldValue:number;
}
export interface ResearchComponentInventory{
 key:string;itemId?:number;itemName:string;iconId?:number;kind:ResearchComponentKind;held:number;
 holders:ResearchHolding[];recipes:ResearchRecipeView[];valuePp?:number;valueBasis?:string;
}

const normalized=(value:string)=>value.trim().toLocaleLowerCase().replace(/[\x60\u2018\u2019]/g,"'").replace(/\s+/g," ");
const itemKeys=(itemId:number|undefined,name:string)=>{
 const keys=[`name:${normalized(name)}`];if(itemId!=null)keys.unshift(`id:${itemId}`);return keys;
};

function inventoryHoldings(inventory:InventoryItem[]){
 const result=new Map<string,Map<string,ResearchHolding>>();
 for(const item of inventory){
  for(const key of new Set(itemKeys(item.itemId,item.itemName))){
   if(!result.has(key))result.set(key,new Map());
   const people=result.get(key)!;
   const current=people.get(item.character)||{character:item.character,count:0,locations:[]};
   current.count+=item.count;if(!current.locations.includes(item.location))current.locations.push(item.location);
   people.set(item.character,current);
  }
 }
 return result;
}

export function buildResearchRecipes(catalog:ResearchCatalogItem[],inventory:InventoryItem[]):ResearchRecipeView[]{
 const holdings=inventoryHoldings(inventory),grouped=new Map<number,ResearchCatalogItem[]>();
 for(const row of catalog)grouped.set(row.recipeId,[...(grouped.get(row.recipeId)||[]),row]);
 return [...grouped].map(([recipeId,rows])=>{
  const first=rows[0];
  const components=rows.map(row=>{
   const people=itemKeys(row.itemId,row.itemName).map(key=>holdings.get(key)).find(Boolean)||new Map<string,ResearchHolding>();
   const holders=[...people.values()].sort((a,b)=>a.character.localeCompare(b.character));
   return{...row,holders,held:holders.reduce((sum,holder)=>sum+holder.count,0)};
  });
  const ownedRequirements=components.filter(component=>component.held>=component.quantity).length;
  const craftableCopies=components.length?Math.min(...components.map(component=>Math.floor(component.held/component.quantity))):0;
  return{key:`research:${recipeId}`,recipeId,className:first.className,level:first.level,spellName:first.spellName,
   spellItemId:first.spellItemId,spellValuePp:first.spellValuePp,trivial:first.trivial,
   researchOnly:first.researchOnly,availability:first.availability,sourceUrl:first.sourceUrl,components,
   ownedRequirements,totalRequirements:components.length,ready:components.length>0&&ownedRequirements===components.length,
   craftableCopies,heldValue:components.reduce((sum,component)=>sum+component.held*(component.valuePp||0),0)};
 }).sort((a,b)=>a.className.localeCompare(b.className)||a.level-b.level||a.spellName.localeCompare(b.spellName));
}

export function buildResearchComponentInventory(recipes:ResearchRecipeView[]):ResearchComponentInventory[]{
 const grouped=new Map<string,ResearchComponentInventory>();
 for(const recipe of recipes)for(const component of recipe.components){
  const key=component.itemId!=null?`id:${component.itemId}`:`name:${normalized(component.itemName)}`;
  const current=grouped.get(key);
  if(current){if(!current.recipes.some(row=>row.recipeId===recipe.recipeId))current.recipes.push(recipe);continue}
  grouped.set(key,{key,itemId:component.itemId,itemName:component.itemName,iconId:component.iconId,kind:component.componentKind,
   held:component.held,holders:component.holders,recipes:[recipe],valuePp:component.valuePp,valueBasis:component.valueBasis});
 }
 return [...grouped.values()].sort((a,b)=>b.held-a.held||b.recipes.length-a.recipes.length||a.itemName.localeCompare(b.itemName));
}