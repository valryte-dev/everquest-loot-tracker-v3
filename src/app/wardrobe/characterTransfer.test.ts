import {describe,expect,it,vi} from "vitest";
import type {InventoryItem,WardrobeCatalogItem} from "../../shared/contracts";
import {createCharacterWardrobeTransfer} from "./characterTransfer";

const inventory=(id:number,location:string,itemName:string,itemId=id):InventoryItem=>({character:"Valmonk",importedAt:"",id,location,itemName,itemId,count:1});
const catalog=(id:number,name:string):WardrobeCatalogItem=>({id,peqId:id,name,noDrop:false,slots:0,classes:0,races:0,weight:0,ac:0,hp:0,mana:0,strength:0,stamina:0,agility:0,dexterity:0,intelligence:0,wisdom:0,charisma:0,magicResist:0,fireResist:0,coldResist:0,diseaseResist:0,poisonResist:0,attack:0,haste:0,manaRegen:0,damageShield:0,damage:0,delay:0,clickName:"",procName:"",wornName:"",focusName:"",setNames:[]});

describe("character wardrobe transfer",()=>{
 it("copies the profile and preserves paired-slot occurrences",async()=>{
  const lookup=vi.fn(async(id:number|undefined,name:string)=>catalog(id||0,name));
  const result=await createCharacterWardrobeTransfer(
   {race:"ik",gender:"m",classCode:"mnk"},
   [inventory(1,"Ear","First Earring"),inventory(2,"Ear","Second Earring"),inventory(3,"Primary","Mace")],
   lookup,
  );
  expect(result.draft.profile).toEqual({race:"ik",gender:"m",classCode:"mnk"});
  expect(result.draft.items["left-ear"]?.name).toBe("First Earring");
  expect(result.draft.items["right-ear"]?.name).toBe("Second Earring");
  expect(result.draft.items.primary?.name).toBe("Mace");
  expect(result.copied).toBe(3);
 });

 it("reports catalog misses without dropping items that resolve",async()=>{
  const result=await createCharacterWardrobeTransfer(
   {race:"hu",gender:"f",classCode:"clr"},
   [inventory(1,"Head","Known"),inventory(2,"Chest","Missing")],
   async(id,name)=>id===1?catalog(id,name):null,
  );
  expect(result.copied).toBe(1);
  expect(result.missing).toEqual(["Missing"]);
 });
});