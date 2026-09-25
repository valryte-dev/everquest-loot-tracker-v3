import {describe,expect,it} from "vitest";
import type {WardrobeCatalogItem} from "../../shared/contracts";
import {classBit,filterByStats,filterByTradeability,filterByWeaponHandedness,raceBit,SLOT_BITS,sortByWardrobeStats,wardrobeTotals,WARDROBE_CHOOSER_STATS,WARDROBE_STATS} from "./model";

const item=(overrides:Partial<WardrobeCatalogItem>={}):WardrobeCatalogItem=>({id:1,name:"Test",noDrop:false,slots:SLOT_BITS.head,classes:1,races:1,weight:0,ac:10,hp:25,mana:0,strength:2,stamina:0,agility:0,dexterity:0,intelligence:0,wisdom:0,charisma:0,magicResist:0,fireResist:0,coldResist:0,diseaseResist:0,poisonResist:0,attack:0,haste:0,manaRegen:0,damageShield:0,damage:0,delay:0,clickName:"",procName:"",wornName:"",focusName:"",setNames:[],...overrides});

describe("wardrobe model",()=>{
 it("maps profile and paired equipment slots to canonical bitmasks",()=>{
  expect(classBit("war")).toBe(1);expect(raceBit("ik")).toBe(4096);
  expect(SLOT_BITS["left-wrist"]).not.toBe(SLOT_BITS["right-wrist"]);
 });
 it("applies every active stat filter",()=>{
  expect(filterByStats([item(),item({id:2,ac:4,hp:100})],[{key:"ac",operator:">=",value:8},{key:"hp",operator:"<=",value:30}]).map(row=>row.id)).toEqual([1]);
 });
 it("totals equipped stats",()=>expect(wardrobeTotals([item(),item({id:2,ac:5,hp:10})])).toMatchObject({ac:15,hp:35,strength:4}));
 it("gives every chooser stat its own compact sortable column",()=>{
  expect(new Set(WARDROBE_CHOOSER_STATS.map(column=>column.key))).toEqual(new Set(["weight","ratio",...WARDROBE_STATS.map(stat=>stat.key)]));
  expect(WARDROBE_CHOOSER_STATS.every(column=>column.label.length<=4)).toBe(true);
 });
 it("filters catalog items by authoritative tradeability",()=>{
  const rows=[item({id:1,noDrop:false}),item({id:2,noDrop:true})];
  expect(filterByTradeability(rows,"tradable").map(row=>row.id)).toEqual([1]);
  expect(filterByTradeability(rows,"no-drop").map(row=>row.id)).toEqual([2]);
 });
 it("filters weapon handedness from canonical item types",()=>{
  const rows=[item({id:1,itemType:0}),item({id:2,itemType:4}),item({id:3,itemType:10})];
  expect(filterByWeaponHandedness(rows,"1h").map(row=>row.id)).toEqual([1]);
  expect(filterByWeaponHandedness(rows,"2h").map(row=>row.id)).toEqual([2]);
 });
 it("sorts weapon ratio as damage divided by delay",()=>{
  expect(sortByWardrobeStats([item({id:1,name:"Slow",damage:10,delay:30}),item({id:2,name:"Efficient",damage:12,delay:24})],[{key:"ratio",direction:"desc"}]).map(row=>row.name)).toEqual(["Efficient","Slow"]);
 });
 it("sorts by ordered stat priorities",()=>{
  expect(sortByWardrobeStats(
   [item({id:1,name:"Balanced",ac:10,hp:20}),item({id:2,name:"Tank",ac:10,hp:40}),item({id:3,name:"Caster",ac:20,hp:5})],
   [{key:"ac",direction:"desc"},{key:"hp",direction:"desc"}],
  ).map(value=>value.name)).toEqual(["Caster","Tank","Balanced"]);
 });
});
