import {describe,expect,it} from "vitest";
import type {DotTrainingReport} from "../../shared/contracts";
import {buildTrainingReplayFrames,createTrainingReplayTimeline,replayCursorAt} from "./trainingReplay";

const report={encounters:[{id:1,mobName:"a golem",startedAt:"2026-09-06 10:00:00",lastDamageAt:"2026-09-06 10:00:06",totalDamage:245,meleeDamage:80,spellDamage:165,hitCount:3,outcome:"active",participants:[],dots:[{id:1,encounterId:1,spellName:"Dawncall",targetName:"a golem",casterName:"Cleric",attributionMethod:"direct_cast",landedAt:"2026-09-06 10:00:00",expiresAt:"2026-09-06 10:00:36",damagePerTick:125,tickIntervalSeconds:6,totalTicks:6,ticksApplied:1,inferenceEnabled:true,status:"active",sourceOffset:1}],procs:[],events:[
 {id:2,encounterId:1,happenedAt:"2026-09-06 10:00:06",attacker:"Cleric",damageType:"spell",attack:"Dawncall",damage:125,inferred:true,sourceKind:"dot",tickIndex:1,sourceOffset:4},
 {id:1,encounterId:1,happenedAt:"2026-09-06 10:00:01",attacker:"Warrior",damageType:"melee",attack:"slash",damage:80,inferred:false,sourceKind:"explicit",sourceOffset:2},
 {id:3,encounterId:1,happenedAt:"2026-09-06 10:00:06",attacker:"Cleric",damageType:"spell",attack:"non-melee",damage:40,inferred:false,sourceKind:"explicit",sourceOffset:5},
],incomingEvents:[
 {id:4,encounterId:1,happenedAt:"2026-09-06 10:00:02",attacker:"a golem",target:"Warrior",attack:"crush",damage:35,sourceOffset:3},
]}],activeCharacter:"Warrior",lineCount:4,recognizedCount:4,ignoredCount:0,dotProfileCount:1,combatProfileCount:1,projectedAllTicks:false,lines:[],warnings:[]} as DotTrainingReport;

describe("DoT training replay",()=>{
 it("orders outgoing and incoming damage chronologically",()=>expect(createTrainingReplayTimeline(report).map(event=>[event.direction,event.id])).toEqual([["outgoing",1],["incoming",4],["outgoing",2],["outgoing",3]]));
 it("finds all events at the selected replay time",()=>{const timeline=createTrainingReplayTimeline(report);expect(replayCursorAt(timeline,Date.parse("2026-09-06T10:00:06"))).toBe(4)});
 it("builds cumulative colored-bar data without leaking future events",()=>{
  const first=buildTrainingReplayFrames(report,1)[0];
  expect(first.totalDamage).toBe(80);
  expect(first.fighters.map(fighter=>fighter.name)).toEqual(["Warrior"]);
  expect(first.fighters[0].incomingDamage).toBe(0);
  const afterIncoming=buildTrainingReplayFrames(report,2)[0];
  expect(afterIncoming.fighters[0].incomingDamage).toBe(35);
  const finished=buildTrainingReplayFrames(report,4)[0];
  expect(finished.totalDamage).toBe(245);
  expect(finished.fighters[0]).toMatchObject({name:"Cleric",totalDamage:165,spellCount:2,spellDirectDamage:40,spellDotDamage:125});
 });
 it("keeps incoming-only players and pets in the damage-taken ranking",()=>{
  const withIncomingOnly={...report,encounters:report.encounters.map(encounter=>({...encounter,incomingEvents:[...encounter.incomingEvents,{id:5,encounterId:1,happenedAt:"2026-09-06 10:00:03",attacker:"a golem",target:"Cleric pet",attack:"bash",damage:60,sourceOffset:6}]}))} as DotTrainingReport;
  const frame=buildTrainingReplayFrames(withIncomingOnly,createTrainingReplayTimeline(withIncomingOnly).length)[0];
  expect(frame.incomingTargets.map(target=>[target.name,target.totalDamage,target.hitCount,target.maxHit])).toEqual([
   ["Warrior",35,1,35],
   ["Cleric pet",60,1,60],
  ]);
 });
});