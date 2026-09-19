import type {InventoryItem,QuestCatalogItem} from "../../shared/contracts";

export interface QuestHolding {character:string;count:number;locations:string[]}
export interface QuestComponentView extends QuestCatalogItem {held:number;holders:QuestHolding[]}
export interface QuestCardView {
 key:string;category:QuestCatalogItem["category"];className:string;questName:string;
 rewardName:string;rewardIconId?:number;sourceUrl:string;components:QuestComponentView[];
 owned:number;required:number;ready:boolean;estimatedHeldValue:number;
}
export interface QuestCardGroup {key:string;label:string;cards:QuestCardView[]}

export const VELIOUS_FACTIONS=["Thurgadin","Skyshrine","Kael"] as const;
export const VELIOUS_ARMOR_TYPES=["Plate","Chain","Leather","Silk"] as const;
const CLASS_ARMOR_TYPE:Record<string,typeof VELIOUS_ARMOR_TYPES[number]>={
 Bard:"Chain",Cleric:"Plate",Druid:"Leather",Enchanter:"Silk",Magician:"Silk",
 Monk:"Leather",Necromancer:"Silk",Paladin:"Plate",Ranger:"Chain",Rogue:"Chain",
 "Shadow Knight":"Plate",Shaman:"Chain",Warrior:"Plate",Wizard:"Silk"
};

const normalized=(value:string)=>value.trim().toLocaleLowerCase().replace(/[\x60\u2018\u2019]/g,"'");
const itemKey=(itemId:number|undefined,name:string)=>itemId!=null?`id:${itemId}`:`name:${normalized(name)}`;

export const questCardFaction=(card:QuestCardView)=>card.components.find(component=>component.faction)?.faction||"Other";

export function buildVeliousArmorTypeCards(cards:QuestCardView[]):QuestCardView[]{
 const groups=new Map<string,QuestCardView[]>();
 for(const card of cards.filter(card=>card.category==="velious_armor")){
  const faction=questCardFaction(card),armorType=CLASS_ARMOR_TYPE[card.className]||"Silk";
  const key=`velious-type|${faction}|${armorType}`;
  groups.set(key,[...(groups.get(key)||[]),card]);
 }
 return [...groups].map(([key,group])=>{
  const first=group[0],faction=questCardFaction(first),armorType=CLASS_ARMOR_TYPE[first.className]||"Silk";
  const components=[...new Map(group.flatMap(card=>card.components).map(component=>[
   itemKey(component.itemId,component.itemName),component
  ])).values()].sort((a,b)=>a.slot.localeCompare(b.slot)||a.itemName.localeCompare(b.itemName));
  const owned=components.filter(component=>component.held>=component.quantity).length;
  return {key,category:"velious_armor",className:armorType,questName:`${faction} ${armorType} armor`,
   rewardName:`${faction} ${armorType} armor`,rewardIconId:first.rewardIconId,sourceUrl:first.sourceUrl,
   components,owned,required:components.length,ready:components.length>0&&owned===components.length,
   estimatedHeldValue:components.reduce((sum,component)=>sum+component.held*(component.valuePp||0),0)};
 });
}

export function groupQuestCards(cards:QuestCardView[],category:QuestCatalogItem["category"]):QuestCardGroup[]{
 if(category==="plane_of_sky"){
  return [...new Set(cards.map(card=>card.className))].sort().map(className=>({
   key:`class:${className}`,label:className,cards:cards.filter(card=>card.className===className)
  }));
 }
 if(category==="velious_armor"){
  const present=[...new Set(cards.map(questCardFaction))];
  const ordered=[...VELIOUS_FACTIONS.filter(faction=>present.includes(faction)),...present.filter(faction=>!VELIOUS_FACTIONS.includes(faction as typeof VELIOUS_FACTIONS[number])).sort()];
  return ordered.map(faction=>({key:`faction:${faction}`,label:faction,cards:cards.filter(card=>questCardFaction(card)===faction)}));
 }
 return cards.length?[{key:"epic",label:"",cards}]:[];
}

export function buildQuestCards(catalog:QuestCatalogItem[],inventory:InventoryItem[]):QuestCardView[]{
 const holdings=new Map<string,Map<string,QuestHolding>>();
 for(const item of inventory){
  const keys=new Set([itemKey(item.itemId,item.itemName),`name:${normalized(item.itemName)}`]);
  for(const key of keys){
   if(!holdings.has(key))holdings.set(key,new Map());
   const byCharacter=holdings.get(key)!;
   const current=byCharacter.get(item.character)||{character:item.character,count:0,locations:[]};
   current.count+=item.count;
   if(!current.locations.includes(item.location))current.locations.push(item.location);
   byCharacter.set(item.character,current);
  }
 }
 const groups=new Map<string,QuestCatalogItem[]>();
 for(const row of catalog){
  const key=row.category==="velious_armor"
   ?`${row.category}|${row.className}|${row.questName}`
   :`${row.category}|${row.entryId}`;
  groups.set(key,[...(groups.get(key)||[]),row]);
 }
 return [...groups].map(([key,rows])=>{
  const first=rows[0];
  const unique=[...new Map(rows.map(row=>[normalized(row.itemName),row])).values()];
  const components=unique.map(row=>{
   const byCharacter=holdings.get(itemKey(row.itemId,row.itemName))
    ||holdings.get(`name:${normalized(row.itemName)}`)||new Map();
   const holders=[...byCharacter.values()].sort((a,b)=>a.character.localeCompare(b.character));
   return {...row,holders,held:holders.reduce((sum,holder)=>sum+holder.count,0)};
  });
  const owned=components.filter(component=>component.held>=component.quantity).length;
  return {key,category:first.category,className:first.className,questName:first.questName,
   rewardName:first.category==="velious_armor"?first.questName:first.rewardName,
   rewardIconId:first.rewardIconId,sourceUrl:first.sourceUrl,components,owned,
   required:components.length,ready:components.length>0&&owned===components.length,
   estimatedHeldValue:components.reduce((sum,component)=>sum+component.held*(component.valuePp||0),0)};
 });
}
