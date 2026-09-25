import {describe,expect,it} from "vitest";
import type {WardrobeCatalogItem} from "../../shared/contracts";
import {itemClasses,itemRaces,itemSlots,itemStats} from "./ItemHover";

const fungi:WardrobeCatalogItem={id:1,name:"Fungus Covered Scale Tunic",noDrop:false,slots:131072,classes:1023,races:8191,weight:20,ac:21,hp:0,mana:0,strength:2,stamina:0,agility:-10,dexterity:-10,intelligence:2,wisdom:0,charisma:0,magicResist:0,fireResist:0,coldResist:0,diseaseResist:0,poisonResist:0,attack:0,haste:0,manaRegen:0,damageShield:0,damage:0,delay:0,clickName:"",procName:"",wornName:"Fungal Regrowth",focusName:"",setNames:[]};

describe("item hover summary",()=>{
 it("decodes the P99 equipment masks",()=>{
  expect(itemSlots(fungi.slots)).toBe("CHEST");
  expect(itemClasses(16383)).toBe("ALL");
  expect(itemRaces(fungi.races)).toBe("ALL");
 });
 it("keeps signed positive and negative item attributes",()=>{
  expect(itemStats(fungi).attributes.map(row=>[row.label,row.value])).toEqual([["STR",2],["AGI",-10],["DEX",-10],["INT",2]]);
 });
});
