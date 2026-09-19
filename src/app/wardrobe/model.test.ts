import {describe,expect,it} from "vitest";
import type {WardrobeCatalogItem} from "../../shared/contracts";
import {classBit,filterByStats,raceBit,SLOT_BITS,wardrobeTotals} from "./model";

const item=(overrides:Partial<WardrobeCatalogItem>={}):WardrobeCatalogItem=>({id:1,name:"Test",slots:SLOT_BITS.head,classes:1,races:1,weight:0,ac:10,hp:25,mana:0,strength:2,stamina:0,agility:0,dexterity:0,intelligence:0,wisdom:0,charisma:0,magicResist:0,fireResist:0,coldResist:0,diseaseResist:0,poisonResist:0,attack:0,haste:0,manaRegen:0,damageShield:0,damage:0,delay:0,clickName:"",procName:"",wornName:"",focusName:"",setNames:[],...overrides});

describe("wardrobe model",()=>{
 it("maps profile and paired equipment slots to canonical bitmasks",()=>{
  expect(classBit("war")).toBe(1);expect(raceBit("ik")).toBe(4096);
  expect(SLOT_BITS["left-wrist"]).not.toBe(SLOT_BITS["right-wrist"]);
 });
 it("applies every active stat filter",()=>{
  expect(filterByStats([item(),item({id:2,ac:4,hp:100})],[{key:"ac",operator:">=",value:8},{key:"hp",operator:"<=",value:30}]).map(row=>row.id)).toEqual([1]);
 });
 it("totals equipped stats",()=>expect(wardrobeTotals([item(),item({id:2,ac:5,hp:10})])).toMatchObject({ac:15,hp:35,strength:4}));
});
