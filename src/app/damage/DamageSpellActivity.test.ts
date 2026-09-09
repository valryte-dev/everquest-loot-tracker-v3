import {describe,expect,it} from "vitest";
import type {DamageSpellMetric,TrackedSpellActivity} from "../../shared/contracts";
import {rankSpellPlayers,spellSourceLabel} from "./DamageSpellActivity";

const metric=(playerName:string,spellName:string,directProcDamage:number,dotDamage:number):DamageSpellMetric=>({
 playerName,spellName,procCount:0,directProcDamage,dotDamage,dotTickCount:0,procDotDamage:0,totalProcDamage:directProcDamage,
});
const activity=(id:number,casterName:string,spellName:string,sourceKind:TrackedSpellActivity["sourceKind"],sourceName?:string):TrackedSpellActivity=>({
 id,casterName,spellName,sourceKind,sourceName,targetName:"a golem",happenedAt:"2026-09-07 10:00:00",
});

describe("spell activity rankings",()=>{
 it("ranks players by known spell damage and counts each source without overlap",()=>{
  const rows=rankSpellPlayers(
   [metric("Bakamore","Dawncall",0,750),metric("Judoku","Essence Tap",40,0)],
   [activity(1,"Bakamore","Dawncall","direct"),activity(2,"Bakamore","Dawncall","item_click","Great Spear of Dawn"),activity(3,"Judoku","Essence Tap","proc")],
  );
  expect(rows.map(row=>[row.rank,row.name,row.knownDamage,row.landings,row.direct,row.procs,row.itemClicks])).toEqual([
   [1,"Bakamore",750,2,1,0,1],
   [2,"Judoku",40,1,0,1,0],
  ]);
 });
 it("describes item clicks with the retained glowing item",()=>{
  expect(spellSourceLabel(activity(1,"Bakamore","Dawncall","item_click","Great Spear of Dawn"))).toBe("Item click - Great Spear of Dawn");
 });
 it("keeps a known item effect explicitly unattributed",()=>{
  expect(spellSourceLabel(activity(2,"Unattributed","Curse of the Spirits","unknown","Spear of Fate"))).toBe("Unattributed spell - possible Spear of Fate item click");
 });
});