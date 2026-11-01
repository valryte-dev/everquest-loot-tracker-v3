import {describe,expect,it} from "vitest";
import type {InventoryItem,Spell,SpellInfo} from "../../shared/contracts";
import {buildCharacterSpellRows,normalizeSpellName,spellIconPath} from "./spellCollectionModel";

const spell=(spellName:string,level:number):SpellInfo=>({spellName,wikiUrl:"",description:"",iconLetter:"G",classes:[{name:"Cleric",level}],effects:[],mana:"50",skill:"Alteration",castingTime:"2.5",recastTime:"",fizzleTime:"",resist:"Magic",range:"100",targetType:"Single",spellType:"Beneficial",duration:"6 ticks",reagent:"",focus:"",whereToObtain:"Vendor",castOnYou:"",castOnOther:"",wearsOff:"",damageKind:"non_damage",tickIntervalSeconds:6,fetchedAt:"",stale:false});
const known=(spellName:string,slot:number):Spell=>({character:"Test",importedAt:"",spellName,slot});
const scroll=(itemName:string,count=1):InventoryItem=>({id:1,character:"Test",importedAt:"",itemName,location:"Bank",count});

describe("character spellbook model",()=>{
 it("builds only safe bundled spell-icon paths",()=>{
  expect(spellIconPath("g")).toBe("/spell-icons/Spellicon_G.png");
  expect(spellIconPath("../G")).toBeUndefined();
 });

 it("normalizes scroll prefixes and apostrophe variants",()=>{
  expect(normalizeSpellName(" Spell: Yaeth's Compel. ")).toBe("yaeth's compel");
  expect(normalizeSpellName("Yaeth’s Compel")).toBe("yaeth's compel");
 });

 it("classifies scribed, held-scroll, and missing spells",()=>{
  const rows=buildCharacterSpellRows([spell("Courage",1),spell("Center",4),spell("Light Healing",5)],"Cleric",[known("Courage",12)],[scroll("Spell: Center",2)]);
  expect(rows.map(row=>[row.info.spellName,row.status,row.scrollCount,row.bookSlot])).toEqual([
   ["Courage","scribed",0,12],
   ["Center","scroll",2,undefined],
   ["Light Healing","missing",0,undefined],
  ]);
 });

 it("only includes spells for the selected class",()=>{
  const cleric=spell("Courage",1),wizard={...spell("Shock of Frost",1),classes:[{name:"Wizard",level:1}]};
  expect(buildCharacterSpellRows([cleric,wizard],"Cleric",[],[]).map(row=>row.info.spellName)).toEqual(["Courage"]);
 });
});