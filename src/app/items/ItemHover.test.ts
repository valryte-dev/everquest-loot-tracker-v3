import {createElement} from "react";
import {renderToStaticMarkup} from "react-dom/server";
import {describe,expect,it} from "vitest";
import type {WardrobeCatalogItem} from "../../shared/contracts";
import {ItemHover,itemClasses,itemHoverCacheKey,itemRaces,itemSlots,itemStats} from "./ItemHover";

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
 it("keeps item links clickable inside the shared hover target",()=>{
  const html=renderToStaticMarkup(createElement(ItemHover,{itemName:"Fungus Covered Scale Tunic",itemId:123,className:"linked-item",
   children:createElement("a",{href:"https://wiki.project1999.com/Fungus_Covered_Scale_Tunic"},"Fungus Covered Scale Tunic")}));
  expect(html).toContain('class="item-hover-anchor linked-item"');
  expect(html).toContain('href="https://wiki.project1999.com/Fungus_Covered_Scale_Tunic"');
  expect(html).toContain("Fungus Covered Scale Tunic");
 });
 it("does not share cached details between different names with the same id",()=>{
  expect(itemHoverCacheKey(6832,"Part of a Large Key")).not.toBe(itemHoverCacheKey(6832,"Spell: Quivering Veil of Xarn"));
 });
});
