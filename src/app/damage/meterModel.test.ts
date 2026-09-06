import {describe,expect,it} from "vitest";
import type {DamageEvent,DamageParticipant} from "../../shared/contracts";
import {buildMeterTrend,rankMeterPlayers} from "./meterModel";

const player=(name:string,totalDamage:number):DamageParticipant=>({
 name,totalDamage,hitCount:2,firstDamageAt:"2026-09-05 10:00:00",lastDamageAt:"2026-09-05 10:00:10",
});
const event=(id:number,second:number,attacker:string,damage:number):DamageEvent=>({
 id,happenedAt:"2026-09-05 10:00:"+String(second).padStart(2,"0"),attacker,
 damageType:"melee",attack:"hit",damage,
});

describe("damage meter model",()=>{
 it("ranks players and uses the shared encounter duration for DPS",()=>{
  const rows=rankMeterPlayers([player("Two",250),player("One",750)],1000,10);
  expect(rows.map(row=>[row.rank,row.name,row.contribution,row.dps])).toEqual([
   [1,"One",75,75],
   [2,"Two",25,25],
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
 it("ignores players outside the selected trend",()=>{
  const points=buildMeterTrend([event(1,1,"Pet",999)],"2026-09-05 10:00:00",["One"]);
  expect(points).toEqual([{second:0,totals:{One:0}}]);
 });
});
