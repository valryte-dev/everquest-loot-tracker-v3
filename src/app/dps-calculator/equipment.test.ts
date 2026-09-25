import {describe,expect,it} from "vitest";
import type {InventoryItem,WardrobeCatalogItem} from "../../shared/contracts";
import {loadWardrobeDpsEquipment,resolveCharacterDpsEquipment,summarizeDpsEquipment} from "./equipment";
const catalog=(id:number,name:string,overrides:Partial<WardrobeCatalogItem>={}):WardrobeCatalogItem=>({id,peqId:id,name,noDrop:false,slots:0,classes:0,races:0,weight:0,ac:0,hp:0,mana:0,strength:0,stamina:0,agility:0,dexterity:0,intelligence:0,wisdom:0,charisma:0,magicResist:0,fireResist:0,coldResist:0,diseaseResist:0,poisonResist:0,attack:0,haste:0,manaRegen:0,damageShield:0,damage:0,delay:0,clickName:"",procName:"",wornName:"",focusName:"",setNames:[],...overrides});
const inventory=(id:number,location:string,itemName:string):InventoryItem=>({character:"Derpsi",importedAt:"",id,location,itemName,count:1});
describe("DPS equipment loading",()=>{
 it("summarizes strength, non-stacking worn haste, and true weapon handedness",()=>{
  const result=summarizeDpsEquipment({head:catalog(1,"Helm",{strength:5,haste:20}),chest:catalog(2,"Chest",{strength:8,haste:36}),primary:catalog(3,"Staff",{itemType:4,damage:29,delay:30})});
  expect(result).toMatchObject({gearStrength:13,haste:36,mainHand:"2h"});
 });
 it("resolves equipped inventory through the shared catalog boundary",async()=>{
  const rows=[inventory(1,"Primary","Sword"),inventory(2,"Secondary","Dagger")];
  const result=await resolveCharacterDpsEquipment(rows,async(_id,name)=>catalog(name==="Sword"?1:2,name,{damage:10,delay:20}));
  expect(result.primary?.name).toBe("Sword");expect(result.secondary?.name).toBe("Dagger");
 });
 it("loads an offline wardrobe draft",()=>{
  const primary=catalog(3,"Staff",{itemType:4,damage:29,delay:30});
  expect(loadWardrobeDpsEquipment(JSON.stringify({profile:{race:"hu",gender:"m",classCode:"mnk"},items:{primary}})).loadout?.mainHand).toBe("2h");
 });
});