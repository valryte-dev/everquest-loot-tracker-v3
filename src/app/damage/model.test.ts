import {describe,expect,it} from "vitest";
import type {ClericHealCall,DamageEvent} from "../../shared/contracts";
import {buildClericChainTimeline,buildDamageBurstSeries,buildLiveDpsSeries,dpsMomentum,latestHealChainBoundary,selectLiveEncounters} from "./model";

const event=(id:number,second:number,attacker:string,damage:number):DamageEvent=>({
 id,happenedAt:`2026-09-05 10:00:${String(second).padStart(2,"0")}`,attacker,
 damageType:"melee",attack:"hit",damage,
});

describe("cleric CH chain analytics",()=>{
 const call=(id:number,second:number,clericName:string,callNumber:number):ClericHealCall=>({
  id,happenedAt:"2026-09-05 16:"+String(Math.floor(second/60)).padStart(2,"0")+":"+String(second%60).padStart(2,"0"),
  character:"Youngman",clericName,callNumber,targetName:"Forsure",channel:"guild",
  message:"LoF "+String(callNumber).padStart(3,"0")+" CH - Forsure",sourceFile:"eqlog_Youngman_P1999Green.txt",
 });
 it("tracks chain gaps, each cleric cadence, and starts a new session after inactivity",()=>{
  const rows=buildClericChainTimeline([
   call(1,0,"Bakamore",1),
   call(2,10,"Clerica",2),
   call(3,20,"Bakamore",3),
   call(4,180,"Clerica",1),
  ]);
  expect(rows.map(row=>row.session)).toEqual([0,0,0,1]);
  expect(rows.map(row=>row.gapSeconds)).toEqual([undefined,10,10,undefined]);
  expect(rows[2].clericGapSeconds).toBe(20);
  expect(rows[3].clericGapSeconds).toBeUndefined();
 });
 it("uses a 15-second inactivity boundary and the latest slain mob as a hard boundary",()=>{
  const rows=buildClericChainTimeline([call(1,0,"Bakamore",1),call(2,15,"Clerica",2),call(3,31,"Bakamore",3)]);
  expect(rows.map(row=>row.session)).toEqual([0,0,1]);
  const encounter={
   id:1,character:"Youngman",mobName:"a dragon",startedAt:"2026-09-05 16:00:00",
   lastDamageAt:"2026-09-05 16:00:20",endedAt:"2026-09-05 16:00:20",
   totalDamage:1,meleeDamage:1,spellDamage:0,hitCount:1,maxHit:1,
   outcome:"slain" as const,sourceFile:"eqlog_Youngman_P1999Green.txt",weapons:[],players:[],
  };
  expect(latestHealChainBoundary([encounter],"Youngman",undefined)).toBe(Date.parse("2026-09-05T16:00:20"));
  expect(latestHealChainBoundary([encounter],"Other","2026-09-05T16:00:25")).toBe(Date.parse("2026-09-05T16:00:25"));
 });
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
