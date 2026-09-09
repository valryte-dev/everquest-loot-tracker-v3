import {describe,expect,it} from "vitest";
import type {DamageEncounter,DamageEvent,DamageSpellMetric,DotTrainingEncounter} from "../../shared/contracts";
import {describeDamageEvent,describeProcSpell,describeSpellMetric,describeTrainingEventSource} from "./damageSources";

describe("damage source explanations",()=>{
 it("identifies known direct and DoT proc weapons",()=>{
  expect(describeProcSpell("Essence Tap",20,0).label).toBe("Essence Mace / proc DD / Essence Tap");
  expect(describeProcSpell("Dawncall",0,125).label).toBe("Great Spear of Dawn / proc DoT / Dawncall");
 });
 it("keeps proc and non-proc DoT sources separate",()=>{
  const metric={playerName:"One",spellName:"Dawncall",procCount:1,directProcDamage:0,dotDamage:875,dotTickCount:7,procDotDamage:750,totalProcDamage:750} as DamageSpellMetric;
  expect(describeSpellMetric(metric).map(row=>row.label)).toEqual(["Great Spear of Dawn / proc DoT / Dawncall","Spell or item cast / DoT / Dawncall"]);
 });
 it("labels stale-capable weapon snapshots as candidates instead of facts",()=>{
  const event={attacker:"One",attack:"slash",damageType:"melee",source:"explicit",primaryWeapon:"Sword",secondaryWeapon:"Dagger"} as DamageEvent;
  expect(describeDamageEvent(event).label).toBe("Last known weapons (snapshot may be stale): Sword or Dagger / melee / slash");
  expect(describeDamageEvent(event).confidence).toBe("candidate");
 });
 it("uses training attribution for direct-cast dots",()=>{
  const encounter={dots:[{spellName:"Dawncall",casterName:"One",attributionMethod:"direct_cast"}]} as DotTrainingEncounter;
  const event={attacker:"One",attack:"Dawncall",sourceKind:"dot"} as never;
  expect(describeTrainingEventSource(encounter,event).label).toBe("Direct cast / DoT / Dawncall");
 });
});