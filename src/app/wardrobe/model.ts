import type {InventoryItem,WardrobeCatalogItem} from "../../shared/contracts";
import type {EquipmentSlot,EquipmentSlotKey} from "../characters/EquipmentPaperDoll";
import type {CharacterModelProfile} from "../characters/characterProfile";

export type WardrobeStatKey="ac"|"hp"|"mana"|"strength"|"stamina"|"agility"|"dexterity"|"intelligence"|"wisdom"|"charisma"|"magicResist"|"fireResist"|"coldResist"|"diseaseResist"|"poisonResist"|"attack"|"haste"|"manaRegen"|"damageShield"|"damage"|"delay";
export type WardrobeStatOperator=">="|"<="|"="|">"|"<";
export interface WardrobeStatFilter {key:WardrobeStatKey;operator:WardrobeStatOperator;value:number}

export const WARDROBE_STATS:{key:WardrobeStatKey;label:string}[]=[
 {key:"ac",label:"AC"},{key:"hp",label:"HP"},{key:"mana",label:"Mana"},
 {key:"strength",label:"STR"},{key:"stamina",label:"STA"},{key:"agility",label:"AGI"},{key:"dexterity",label:"DEX"},
 {key:"intelligence",label:"INT"},{key:"wisdom",label:"WIS"},{key:"charisma",label:"CHA"},
 {key:"magicResist",label:"MR"},{key:"fireResist",label:"FR"},{key:"coldResist",label:"CR"},{key:"diseaseResist",label:"DR"},{key:"poisonResist",label:"PR"},
 {key:"attack",label:"ATK"},{key:"haste",label:"Haste"},{key:"manaRegen",label:"Mana regen"},{key:"damageShield",label:"Damage shield"},
 {key:"damage",label:"Weapon damage"},{key:"delay",label:"Delay"},
];

export type WardrobeChooserStatKey=WardrobeStatKey|"weight"|"ratio";
export const WARDROBE_CHOOSER_STATS:{key:WardrobeChooserStatKey;label:string}[]=[
 {key:"weight",label:"WT"},{key:"ac",label:"AC"},{key:"hp",label:"HP"},{key:"mana",label:"MANA"},
 {key:"strength",label:"STR"},{key:"stamina",label:"STA"},{key:"agility",label:"AGI"},{key:"dexterity",label:"DEX"},
 {key:"intelligence",label:"INT"},{key:"wisdom",label:"WIS"},{key:"charisma",label:"CHA"},
 {key:"magicResist",label:"MR"},{key:"fireResist",label:"FR"},{key:"coldResist",label:"CR"},{key:"diseaseResist",label:"DR"},{key:"poisonResist",label:"PR"},
 {key:"attack",label:"ATK"},{key:"haste",label:"HST"},{key:"manaRegen",label:"MREG"},{key:"damageShield",label:"DS"},
 {key:"damage",label:"DMG"},{key:"delay",label:"DLY"},{key:"ratio",label:"RAT"},
];
export type WardrobeTradeability="all"|"tradable"|"no-drop";
export const filterByTradeability=(items:WardrobeCatalogItem[],tradeability:WardrobeTradeability)=>tradeability==="all"?items:items.filter(item=>tradeability==="no-drop"?item.noDrop:!item.noDrop);

export type WeaponHandedness="all"|"1h"|"2h";
const ONE_HANDED_ITEM_TYPES=new Set([0,2,3,45]);
const TWO_HANDED_ITEM_TYPES=new Set([1,4,35]);
export const weaponHandedness=(item:WardrobeCatalogItem):Exclude<WeaponHandedness,"all">|null=>ONE_HANDED_ITEM_TYPES.has(item.itemType??-1)?"1h":TWO_HANDED_ITEM_TYPES.has(item.itemType??-1)?"2h":null;
export const filterByWeaponHandedness=(items:WardrobeCatalogItem[],handedness:WeaponHandedness)=>handedness==="all"?items:items.filter(item=>weaponHandedness(item)===handedness);
export const weaponRatio=(item:WardrobeCatalogItem)=>item.damage>0&&item.delay>0?item.damage/item.delay:0;

export type WardrobeSortDirection="asc"|"desc";
export interface WardrobeStatSort {key:WardrobeChooserStatKey;direction:WardrobeSortDirection}
const wardrobeSortValue=(item:WardrobeCatalogItem,key:WardrobeChooserStatKey)=>key==="ratio"?weaponRatio(item):item[key];
export function sortByWardrobeStats(items:WardrobeCatalogItem[],sorts:WardrobeStatSort[]){
 if(!sorts.length)return items;
 return [...items].sort((left,right)=>{
  for(const sort of sorts){
   const difference=wardrobeSortValue(left,sort.key)-wardrobeSortValue(right,sort.key);
   if(difference)return sort.direction==="desc"?-difference:difference;
  }
  return left.name.localeCompare(right.name,undefined,{numeric:true,sensitivity:"base"});
 });
}
export const SLOT_BITS:Record<EquipmentSlotKey,number>={
 "left-ear":2,head:4,face:8,"right-ear":16,neck:32,shoulders:64,arms:128,back:256,
 "left-wrist":512,"right-wrist":1024,range:2048,hands:4096,primary:8192,secondary:16384,
 "left-finger":32768,"right-finger":65536,chest:131072,legs:262144,feet:524288,waist:1048576,ammo:2097152,
};

const CLASS_BITS:Record<string,number>={war:1,clr:2,pal:4,rng:8,shd:16,dru:32,mnk:64,brd:128,rog:256,shm:512,nec:1024,wiz:2048,mag:4096,enc:8192};
const RACE_BITS:Record<string,number>={hu:1,ba:2,er:4,el:8,hi:16,da:32,ha:64,dw:128,tr:256,og:512,ho:1024,gn:2048,ik:4096};

export const classBit=(code:string)=>CLASS_BITS[code]||0;
export const raceBit=(code:string)=>RACE_BITS[code]||0;
export const statLabel=(key:WardrobeStatKey)=>WARDROBE_STATS.find(stat=>stat.key===key)?.label||key;
export const compareStat=(actual:number,operator:WardrobeStatOperator,expected:number)=>operator===">="?actual>=expected:operator==="<="?actual<=expected:operator===">"?actual>expected:operator==="<"?actual<expected:actual===expected;
export const filterByStats=(items:WardrobeCatalogItem[],filters:WardrobeStatFilter[])=>items.filter(item=>filters.every(filter=>compareStat(item[filter.key],filter.operator,filter.value)));

export function wardrobeItemToInventory(item:WardrobeCatalogItem,slot:EquipmentSlot,index:number):InventoryItem{
 return {character:"Wardrobe",importedAt:"",id:-(index+1),location:slot.label,itemName:item.name,itemId:item.peqId||item.id,iconId:item.iconId,material:item.material,idFile:item.idFile,color:item.color,itemType:item.itemType,count:1,slots:item.slots};
}

export function wardrobeTotals(items:WardrobeCatalogItem[]){
 return WARDROBE_STATS.filter(stat=>stat.key!=="damage"&&stat.key!=="delay").reduce<Record<string,number>>((totals,stat)=>{totals[stat.key]=items.reduce((sum,item)=>sum+item[stat.key],0);return totals},{});
}

export interface WardrobeDraft {profile:CharacterModelProfile;items:Partial<Record<EquipmentSlotKey,WardrobeCatalogItem>>}
export const DEFAULT_WARDROBE_DRAFT:WardrobeDraft={profile:{race:"hu",gender:"m",classCode:"war"},items:{}};
