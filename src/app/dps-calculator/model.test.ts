import {describe,expect,it} from "vitest";
import {calculateDps,damageBonus,suggestedSkills,type DpsCalculatorInput} from "./model";

const input=(overrides:Partial<DpsCalculatorInput>={}):DpsCalculatorInput=>({characterClass:"Warrior",level:60,strength:255,haste:0,dualWieldSkill:252,doubleAttackSkill:252,offenseSkill:252,backstabSkill:0,mainDamage:15,mainDelay:20,mainHand:"1h",offDamage:15,offDelay:20,...overrides});

describe("P99 DPS estimator",()=>{
 it("calculates the published one-hand level bonus",()=>expect(damageBonus(input())).toBe(11));
 it("uses the factual two-hand delay table",()=>expect(damageBonus(input({mainHand:"2h",mainDelay:40,offDamage:0,offDelay:0}))).toBe(34));
 it("ignores retained offhand values while a two-handed weapon is selected",()=>{
  const withOffhand=calculateDps(input({mainHand:"2h",mainDelay:40,offDamage:50,offDelay:10}));
  const withoutOffhand=calculateDps(input({mainHand:"2h",mainDelay:40,offDamage:0,offDelay:0}));
  expect(withOffhand.off.damageCap).toBe(0);expect(withOffhand.totalDps).toBe(withoutOffhand.totalDps);
 });
 it("applies haste caps and aggregates dual, double, and triple attack",()=>{
  const result=calculateDps(input({haste:150}));
  expect(result.hasteCap).toBe(1);expect(result.effectiveHaste).toBe(1);expect(result.totalDps).toBeGreaterThan(result.main.baseDps+result.off.baseDps);
 });
 it("applies class and level damage caps",()=>{
  const result=calculateDps(input({characterClass:"Wizard",level:8,mainDamage:50,offDamage:0}));
  expect(result.main.damageCap).toBe(6);expect(result.warnings).toHaveLength(1);
 });
 it("suggests only class-appropriate skills",()=>{
  expect(suggestedSkills("Wizard",60)).toMatchObject({dualWieldSkill:0,doubleAttackSkill:0,backstabSkill:0});
  expect(suggestedSkills("Rogue",60)).toMatchObject({dualWieldSkill:252,doubleAttackSkill:252,backstabSkill:252});
 });
});