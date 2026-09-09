import {describe,expect,it} from "vitest";
import type {DamageEncounter,DamageEvent,DamageParticipant} from "../../shared/contracts";
import {buildMeterTrend,buildRollingDpsTrend,damageComposition,formatCombatClock,participantCombatSeconds,playerSpellMetrics,rankIncomingTargets,rankMeterPlayers,rankProccers,summarizePlayerEffects,summarizeSpellMetrics} from "./meterModel";

const player=(name:string,totalDamage:number):DamageParticipant=>({
 name,totalDamage,hitCount:2,firstDamageAt:"2026-09-05 10:00:00",lastDamageAt:"2026-09-05 10:00:10",
});
const event=(id:number,second:number,attacker:string,damage:number):DamageEvent=>({
 id,happenedAt:"2026-09-05 10:00:"+String(second).padStart(2,"0"),attacker,
 damageType:"melee",attack:"hit",damage,
});

describe("damage meter model",()=>{
 it("formats combat timers as stable clocks",()=>{
  expect(formatCombatClock(0)).toBe("00:00:00");
  expect(formatCombatClock(3661.9)).toBe("01:01:01");
 });
 it("measures each fighter from their first recorded hit",()=>{
  const row=player("One",100);
  expect(participantCombatSeconds(row,Date.parse("2026-09-05T10:00:25"))).toBe(25);
  expect(participantCombatSeconds(row,Date.parse("2026-09-05T09:59:59"))).toBe(0);
 });
 it("ranks players and uses the shared encounter duration for DPS",()=>{
  const rows=rankMeterPlayers([player("Two",250),player("One",750)],1000,10);
  expect(rows.map(row=>[row.rank,row.name,row.contribution,row.dps])).toEqual([
   [1,"One",75,75],
   [2,"Two",25,25],
  ]);
 });
 it("ranks incoming targets by damage taken with stable shares",()=>{
  const rows=rankIncomingTargets([
   {...player("Pet",200),maxHit:90},
   {...player("Tank",600),maxHit:120},
  ]);
  expect(rows.map(row=>[row.rank,row.name,row.contribution,row.maxHit])).toEqual([
   [1,"Tank",75,120],
   [2,"Pet",25,90],
  ]);
 });
 it("builds cumulative player damage points",()=>{
  const points=buildMeterTrend([
   event(1,1,"One",100),event(2,1,"Two",40),event(3,3,"One",60),
  ],"2026-09-05 10:00:00",["One","Two"]);
  expect(points).toEqual([
   {second:0,totals:{One:0,Two:0}},
   {second:1,totals:{One:100,Two:40}},
   {second:3,totals:{One:160,Two:40}},
  ]);
 });
 it("builds a bounded 30-second rolling DPS series for every selected fighter",()=>{
  const points=buildRollingDpsTrend([
   event(1,1,"One",60),event(2,2,"Two",30),event(3,31,"One",90),event(4,31,"Ignored",999),
  ],"2026-09-05 10:00:00",["One","Two"],31);
  expect(points).toHaveLength(32);
  expect(points.find(point=>point.second===1)?.dps).toEqual({One:60,Two:0});
  expect(points.find(point=>point.second===30)?.dps).toEqual({One:2,Two:1});
  expect(points.at(-1)).toEqual({second:31,dps:{One:3,Two:1}});
 });
 it("limits rolling DPS history without losing the current second",()=>{
  const points=buildRollingDpsTrend([event(1,1,"One",60)],"2026-09-05 10:00:00",["One"],500,30,120);
  expect(points[0].second).toBe(380);
  expect(points.at(-1)?.second).toBe(500);
  expect(points).toHaveLength(121);
 }); it("ignores players outside the selected trend",()=>{
  const points=buildMeterTrend([event(1,1,"Pet",999)],"2026-09-05 10:00:00",["One"]);
  expect(points).toEqual([{second:0,totals:{One:0}}]);
 });
 it("groups proc and dot metrics for one player without mixing aliases",()=>{
  const encounter={spellMetrics:[
   {playerName:"Judoku",spellName:"Essence Tap",procCount:2,directProcDamage:40,dotDamage:0,dotTickCount:0,procDotDamage:0,totalProcDamage:40},
   {playerName:"Judoku",spellName:"Dawncall",procCount:1,directProcDamage:0,dotDamage:750,dotTickCount:6,procDotDamage:750,totalProcDamage:750},
   {playerName:"Other",spellName:"Dawncall",procCount:1,directProcDamage:0,dotDamage:125,dotTickCount:1,procDotDamage:125,totalProcDamage:125},
  ]} as DamageEncounter;
  const metrics=playerSpellMetrics(encounter,"judoku");
  expect(metrics).toHaveLength(2);
  expect(summarizeSpellMetrics(metrics)).toEqual({procCount:3,directProcDamage:40,dotDamage:750,dotTickCount:6,procDotDamage:750,totalProcDamage:790});
 });
 it("ranks proccers by calculated direct and DoT proc damage",()=>{
  const ranked=rankProccers([
   {playerName:"Judoku",spellName:"Essence Tap",procCount:2,directProcDamage:80,dotDamage:0,dotTickCount:0,procDotDamage:0,totalProcDamage:80},
   {playerName:"judoku",spellName:"Dawncall",procCount:1,directProcDamage:0,dotDamage:250,dotTickCount:2,procDotDamage:250,totalProcDamage:250},
   {playerName:"Balbazak",spellName:"Dawncall",procCount:1,directProcDamage:0,dotDamage:125,dotTickCount:1,procDotDamage:125,totalProcDamage:125},
   {playerName:"Caster",spellName:"Direct DoT",procCount:0,directProcDamage:0,dotDamage:500,dotTickCount:4,procDotDamage:0,totalProcDamage:0},
  ]);
  expect(ranked).toEqual([
   {name:"Judoku",rank:1,procCount:3,directDamage:80,dotDamage:250,totalDamage:330,contribution:72.52747252747253},
   {name:"Balbazak",rank:2,procCount:1,directDamage:0,dotDamage:125,totalDamage:125,contribution:27.472527472527474},
  ]);
 }); it("keeps proc and direct-cast damage separate and cumulative",()=>{
  const encounter={spellMetrics:[
   {playerName:"Judoku",spellName:"Essence Tap",procCount:2,directProcDamage:40,dotDamage:0,dotTickCount:0,procDotDamage:0,totalProcDamage:40},
   {playerName:"Judoku",spellName:"Dawncall",procCount:1,directProcDamage:0,dotDamage:875,dotTickCount:7,procDotDamage:750,totalProcDamage:750},
  ]} as DamageEncounter;
  const events:DamageEvent[]=[
   {id:1,happenedAt:"2026-09-06 10:00:01",attacker:"Judoku",damageType:"spell",source:"explicit",attack:"non-melee",damage:300},
   {id:2,happenedAt:"2026-09-06 10:00:02",attacker:"Judoku",damageType:"spell",source:"proc",attack:"Essence Tap",damage:40},
   {id:3,happenedAt:"2026-09-06 10:00:03",attacker:"Judoku",damageType:"spell",source:"dot",attack:"Dawncall",damage:125},
  ];
  expect(summarizePlayerEffects(encounter,events,"judoku")).toEqual({procCount:3,procDirectDamage:40,procDotDamage:750,spellCount:2,spellDirectDamage:300,spellDotDamage:125});
 });
 it("splits spell damage into direct and dot composition",()=>{
  expect(damageComposition({totalDamage:1000,meleeDamage:400,spellDamage:600,dotDamage:250})).toEqual({
   melee:400,direct:350,dot:250,total:1000,meleePercent:40,directPercent:35,dotPercent:25,
  });
 });});
