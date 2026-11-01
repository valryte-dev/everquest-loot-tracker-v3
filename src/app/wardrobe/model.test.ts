import {describe,expect,it} from "vitest";
import type {MasterItem,WardrobeCatalogItem} from "../../shared/contracts";
import {classBit,createWardrobeMarketLookup,filterByStats,filterByTradeability,filterByWeaponHandedness,quickCompareBuild,quickCompareSelectedUpgrades,raceBit,SLOT_BITS,sortByWardrobeStats,wardrobeStatDeltas,wardrobeTotals,wardrobeValueSummary,WARDROBE_CHOOSER_STATS,WARDROBE_STATS,type WardrobeCompareEntry} from "./model";

const item=(overrides:Partial<WardrobeCatalogItem>={}):WardrobeCatalogItem=>({id:1,name:"Test",noDrop:false,slots:SLOT_BITS.head,classes:1,races:1,weight:0,ac:10,hp:25,mana:0,strength:2,stamina:0,agility:0,dexterity:0,intelligence:0,wisdom:0,charisma:0,magicResist:0,fireResist:0,coldResist:0,diseaseResist:0,poisonResist:0,attack:0,haste:0,manaRegen:0,damageShield:0,damage:0,delay:0,clickName:"",procName:"",wornName:"",focusName:"",setNames:[],...overrides});
const master=(overrides:Partial<MasterItem>={}):MasterItem=>({id:1,name:"Test",valuePp:100,valueBasis:"30-day WTS",count30d:3,lastSeen:"",manual:false,source:"market",...overrides});

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
 it("applies enabled quick compare items by priority without mutating the equipped build",()=>{
  const equipped={head:item({id:1,name:"Old Helm",ac:5}),secondary:item({id:9,name:"Shield"})};
  const entries:WardrobeCompareEntry[]=[
   {id:"normal",item:item({id:2,name:"Normal Helm",ac:10}),slotKey:"head",priority:0,enabled:true,note:""},
   {id:"high",item:item({id:3,name:"High Helm",ac:20}),slotKey:"head",priority:1,enabled:true,note:""},
   {id:"off",item:item({id:4,name:"Disabled",ac:99}),slotKey:"chest",priority:1,enabled:false,note:""},
  ];
  const compared=quickCompareBuild(equipped,entries);
  expect(compared.head?.name).toBe("High Helm");
  expect(compared.chest).toBeUndefined();
  expect(equipped.head?.name).toBe("Old Helm");
 });
 it("clears the off hand when a two-handed comparison wins primary",()=>{
  const compared=quickCompareBuild(
   {secondary:item({id:9,name:"Shield"})},
   [{id:"2h",item:item({id:8,name:"Great Staff",itemType:4}),slotKey:"primary",priority:0,enabled:true,note:""}],
  );
  expect(compared.primary?.name).toBe("Great Staff");
  expect(compared.secondary).toBeUndefined();
 });
 it("resolves prices by canonical PEQ id then name without using private catalog ids",()=>{
  const lookup=createWardrobeMarketLookup([
   master({id:6832,name:"Part of a Large Key",valuePp:10}),
   master({id:19309,name:"Spell: Quivering Veil of Xarn",valuePp:500}),
  ]);
  expect(lookup(item({id:6832,peqId:19309,name:"Spell: Quivering Veil of Xarn"}))?.valuePp).toBe(500);
  expect(lookup(item({id:6832,name:"Spell: Quivering Veil of Xarn"}))?.valuePp).toBe(500);
 });
 it("totals priced items and reports missing price coverage",()=>{
  const lookup=createWardrobeMarketLookup([master({id:10,name:"Priced",valuePp:750})]);
  expect(wardrobeValueSummary([item({id:1,peqId:10,name:"Priced"}),item({id:2,name:"Unknown"})],lookup)).toEqual({totalPp:750,pricedCount:1,missingCount:1});
 });
 it("prices only enabled winning upgrades, not lower-priority alternatives",()=>{
  const equipped={head:item({id:1,name:"Old Helm"})};
  const entries:WardrobeCompareEntry[]=[
   {id:"winner",item:item({id:2,name:"Winning Helm"}),slotKey:"head",priority:1,enabled:true,note:""},
   {id:"other",item:item({id:3,name:"Other Helm"}),slotKey:"head",priority:0,enabled:true,note:""},
  ];
  expect(quickCompareSelectedUpgrades(equipped,entries).map(value=>value.name)).toEqual(["Winning Helm"]);
 });
 it("reports only changed item stats against the displaced slot",()=>{
  expect(wardrobeStatDeltas(item({ac:12,hp:20}),item({ac:10,hp:25}))).toMatchObject({ac:2,hp:-5});
  expect(wardrobeStatDeltas(item({ac:12}),item({ac:12}))).not.toHaveProperty("ac");
 });
});
