import {describe,expect,it} from "vitest";
import type {DamageEvent} from "../../shared/contracts";
import {buildDamageBurstSeries,buildLiveDpsSeries,dpsMomentum,selectLiveEncounters} from "./model";

const event=(id:number,second:number,attacker:string,damage:number):DamageEvent=>({
 id,happenedAt:`2026-09-05 10:00:${String(second).padStart(2,"0")}`,attacker,
 damageType:"melee",attack:"hit",damage,
});

describe("live DPS analytics",()=>{
 it("builds independent group and active-character rolling DPS",()=>{
  const points=buildLiveDpsSeries([
   event(1,0,"Youngman",100),
   event(2,1,"Legiteral",50),
   event(3,4,"Youngman",50),
  ],"Youngman","2026-09-05 10:00:00",5,30,5);
  expect(points.at(-1)).toEqual({second:5,group:20,me:10});
  expect(Math.max(...points.map(point=>point.group))).toBe(100);
  expect(Math.max(...points.map(point=>point.me))).toBe(100);
 });

 it("builds per-second group and personal damage bursts",()=>{
  const points=buildDamageBurstSeries([
   event(1,1,"Youngman",100),
   event(2,1,"Legiteral",50),
   event(3,3,"Youngman",25),
  ],"Youngman","2026-09-05 10:00:00",3,30);
  expect(points).toEqual([
   {second:0,group:0,me:0},
   {second:1,group:150,me:100},
   {second:2,group:0,me:0},
   {second:3,group:25,me:25},
  ]);
 });

 it("keeps simultaneous recently updated encounters as separate live cards",()=>{
  const encounter=(id:number,character:string,mobName:string)=>({
   id,character,mobName,startedAt:"2026-09-05 10:00:00",lastDamageAt:"2026-09-05 10:00:05",
   totalDamage:100,meleeDamage:100,spellDamage:0,hitCount:1,maxHit:100,outcome:"active" as const,
   sourceFile:`eqlog_${character}.txt`,weapons:[],players:[],
  });
  const rows=[encounter(1,"Youngman","a frost giant"),encounter(2,"Youngman","an ice giant"),encounter(3,"Other","a spider")];
  const seen=new Map([[1,99000],[2,99500],[3,99900]]);
  expect(selectLiveEncounters(rows,"Youngman",seen,100000,[]).map(row=>row.id)).toEqual([2,1]);
  expect(selectLiveEncounters(rows,"Youngman",seen,100000,[2]).map(row=>row.id)).toEqual([1]);
 });

 it("reports rising, falling, and steady momentum",()=>{
  const base=Array.from({length:7},(_,second)=>({second,group:10,me:10}));
  expect(dpsMomentum(base,"group")).toBe("steady");
  expect(dpsMomentum(base.map((point,index)=>({...point,group:index===6?20:point.group})),"group")).toBe("rising");
  expect(dpsMomentum(base.map((point,index)=>({...point,me:index===6?0:point.me})),"me")).toBe("falling");
 });
});
