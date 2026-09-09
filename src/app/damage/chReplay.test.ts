import {describe,expect,it} from "vitest";
import type {ClericHealCall,DamageEncounter} from "../../shared/contracts";
import {buildClericChainEncounters,calculateClericChainStats,encounterFromReplay,refreshClericChainEncounterStatuses} from "./chReplay";

const call=(id:number,second:number,character="Tester",healer=id%2?"Alpha":"Beta"):ClericHealCall=>({id,happenedAt:`2026-09-08 10:00:${String(second).padStart(2,"0")}`,character,clericName:healer,callNumber:id,targetName:"Tank",channel:"guild",message:"LoF CH",sourceFile:`eqlog_${character}.txt`});
const fight=(id:number,mobName:string,startedAt:string,lastDamageAt:string):DamageEncounter=>({id,character:"Tester",mobName,startedAt,lastDamageAt,totalDamage:1,meleeDamage:1,spellDamage:0,hitCount:1,maxHit:1,outcome:"active",sourceFile:"eqlog_Tester.txt",weapons:[],players:[]});

describe("CH encounter history",()=>{
 it("groups calls by 15-second chain boundaries and assigns the nearest overlapping mob",()=>{
  const rows=buildClericChainEncounters([call(1,0),call(2,9),call(3,30)], [fight(1,"a dragon","2026-09-08 09:59:55","2026-09-08 10:00:12"),fight(2,"a giant","2026-09-08 10:00:25","2026-09-08 10:00:40")]);
  expect(rows.map(row=>[row.targetMob,row.callCount,row.healerCount])).toEqual([["a giant",1,1],["a dragon",2,2]]);
  expect(rows[1].averageGapSeconds).toBe(9);
 });
 it("ends an encounter at a slain-mob boundary even when the next call is within 15 seconds",()=>{
  const slain={...fight(1,"a dragon","2026-09-08 09:59:55","2026-09-08 10:00:05"),endedAt:"2026-09-08 10:00:05",outcome:"slain" as const};
  expect(buildClericChainEncounters([call(1,0),call(2,9)],[slain])).toHaveLength(2);
 });
 it("never combines calls from different log characters",()=>{
  const rows=buildClericChainEncounters([call(1,0,"Tester"),call(2,1,"Other")],[]);
  expect(rows).toHaveLength(2);
 });
 it("finalizes per-cleric population deviation only after the inactivity boundary",()=>{
  const calls=[call(1,0,"Tester","Alpha"),call(2,5,"Tester","Beta"),call(3,10,"Tester","Alpha"),call(4,15,"Tester","Beta"),call(5,22,"Tester","Alpha")];
  const active=buildClericChainEncounters(calls,[],Date.parse("2026-09-08T10:00:25"))[0];
  const concluded=buildClericChainEncounters(calls,[],Date.parse("2026-09-08T10:00:40"))[0];
  expect(active.status).toBe("active");
  expect(concluded.status).toBe("concluded");
  expect(concluded.clericStats.find(row=>row.name==="Alpha")).toMatchObject({callCount:3,intervalCount:2,averageRotationSeconds:11,standardDeviationSeconds:1,shortestRotationSeconds:10,longestRotationSeconds:12});
 });
 it("uses each cleric's own call intervals rather than overall chain gaps",()=>{
  const timeline=buildClericChainEncounters([call(1,0,"Tester","Alpha"),call(2,4,"Tester","Beta"),call(3,10,"Tester","Alpha")],[],Date.parse("2026-09-08T10:01:00"))[0].calls;
  expect(calculateClericChainStats(timeline).find(row=>row.name==="Alpha")?.averageRotationSeconds).toBe(10);
 }); it("refreshes inactivity status without regrouping calls",()=>{
  const grouped=buildClericChainEncounters([call(1,0)],[],0);
  expect(grouped[0].status).toBe("active");
  expect(refreshClericChainEncounterStatuses(grouped,Date.parse("2026-09-08T10:00:16"))[0].status).toBe("concluded");
 });
 it("reconstructs a saved portable replay",()=>{
  const file={formatVersion:1,title:"chain",savedAt:"2026-09-08T10:01:00Z",character:"Tester",targetMob:"a dragon",summary:{startedAt:"2026-09-08 10:00:00",endedAt:"2026-09-08 10:00:09",durationSeconds:9,callCount:2,healerCount:2,averageGapSeconds:9,longestGapSeconds:9},calls:[{...call(1,0),gapSeconds:undefined},{...call(2,9),gapSeconds:9}]};
  expect(encounterFromReplay(file,"saved.eqch.json")).toMatchObject({id:"saved.eqch.json",targetMob:"a dragon",callCount:2});
 });
 it("normalizes null optional gaps written by saved JSON files",()=>{
  const file={formatVersion:1,title:"chain",savedAt:"2026-09-08T10:01:00Z",character:"Tester",targetMob:"a dragon",summary:{startedAt:"2026-09-08 10:00:00",endedAt:"2026-09-08 10:00:09",durationSeconds:9,callCount:2,healerCount:2,averageGapSeconds:9,longestGapSeconds:9},calls:[{...call(1,0),gapSeconds:null,clericGapSeconds:null},{...call(2,9),gapSeconds:9,clericGapSeconds:null}]};
  const encounter=encounterFromReplay(file as unknown as Parameters<typeof encounterFromReplay>[0],"saved.eqch.json");
  expect(encounter.calls[0].gapSeconds).toBeUndefined();
  expect(encounter.calls[0].clericGapSeconds).toBeUndefined();
  expect(encounter.calls[1].gapSeconds).toBe(9);
 });
});