import type {DamageTarget,DotTrainingEvent,DotTrainingIncomingEvent,DotTrainingReport} from "../../shared/contracts";
import {describeTrainingEventSource,type DamageSourceExplanation} from "./damageSources";

export type TrainingReplayEvent=
 | (DotTrainingEvent&{direction:"outgoing";mobName:string;timeMs:number})
 | (DotTrainingIncomingEvent&{direction:"incoming";mobName:string;timeMs:number});

export interface TrainingReplayFighter {
 name:string;
 totalDamage:number;
 incomingDamage:number;
 hitCount:number;
 dps:number;
 combatSeconds:number;
 contribution:number;
 procCount:number;
 procDirectDamage:number;
 procDotDamage:number;
 spellCount:number;
 spellDirectDamage:number;
 spellDotDamage:number;
 sources:DamageSourceExplanation[];
}

export interface TrainingReplayFrame {
 encounterId:number;
 mobName:string;
 startedAt:string;
 currentAt:string;
 totalDamage:number;
 elapsedSeconds:number;
 fighters:TrainingReplayFighter[];
 incomingTargets:DamageTarget[];
}

export const replayTime=(value:string)=>{
 const parsed=Date.parse(value.includes("T")?value:value.replace(" ","T"));
 return Number.isFinite(parsed)?parsed:0;
};

export function createTrainingReplayTimeline(report:DotTrainingReport):TrainingReplayEvent[]{
 return report.encounters.flatMap(encounter=>[
  ...encounter.events.map(event=>({...event,direction:"outgoing" as const,mobName:encounter.mobName,timeMs:replayTime(event.happenedAt)})),
  ...(encounter.incomingEvents||[]).map(event=>({...event,direction:"incoming" as const,mobName:encounter.mobName,timeMs:replayTime(event.happenedAt)})),
 ]).sort((a,b)=>a.timeMs-b.timeMs||a.sourceOffset-b.sourceOffset||a.id-b.id||a.encounterId-b.encounterId);
}

export function replayCursorAt(timeline:TrainingReplayEvent[],absoluteTimeMs:number):number{
 let low=0,high=timeline.length;
 while(low<high){const middle=Math.floor((low+high)/2);if(timeline[middle].timeMs<=absoluteTimeMs)low=middle+1;else high=middle}
 return low;
}

export function buildTrainingReplayFrames(report:DotTrainingReport,cursor:number):TrainingReplayFrame[]{
 const timeline=createTrainingReplayTimeline(report);
 const revealed=timeline.slice(0,Math.max(0,Math.min(cursor,timeline.length)));
 return report.encounters.flatMap(encounter=>{
  const revealedForEncounter=revealed.filter(event=>event.encounterId===encounter.id);
  if(!revealedForEncounter.length)return[];
  const events=revealedForEncounter.filter((event):event is Extract<TrainingReplayEvent,{direction:"outgoing"}>=>event.direction==="outgoing");
  const incoming=revealedForEncounter.filter((event):event is Extract<TrainingReplayEvent,{direction:"incoming"}>=>event.direction==="incoming");
  const currentMs=Math.max(...revealedForEncounter.map(event=>event.timeMs));
  const startedMs=replayTime(encounter.startedAt);
  const elapsedSeconds=Math.max(1,(currentMs-startedMs)/1000);
  const procDots=new Set(encounter.dots.filter(dot=>dot.attributionMethod==="proc").map(dot=>`${dot.casterName.toLowerCase()}\u001f${dot.spellName.toLowerCase()}`));
  const directDots=encounter.dots.filter(dot=>dot.attributionMethod!=="proc"&&replayTime(dot.landedAt)<=currentMs);
  const names=[...new Set(events.map(event=>event.attacker))];
  const fighters=names.map(name=>{
   const own=events.filter(event=>event.attacker.toLowerCase()===name.toLowerCase());
   const received=incoming.filter(event=>event.target.toLowerCase()===name.toLowerCase());
   const totalDamage=own.reduce((sum,event)=>sum+event.damage,0);
   const firstMs=Math.min(...own.map(event=>event.timeMs));
   const combatSeconds=Math.max(0,(currentMs-firstMs)/1000);
   const procCount=encounter.procs.filter(proc=>proc.casterName.toLowerCase()===name.toLowerCase()&&replayTime(proc.happenedAt)<=currentMs).length;
   const procDirectDamage=own.filter(event=>event.sourceKind==="proc").reduce((sum,event)=>sum+event.damage,0);
   const procDotDamage=own.filter(event=>event.sourceKind==="dot"&&procDots.has(`${name.toLowerCase()}\u001f${event.attack.toLowerCase()}`)).reduce((sum,event)=>sum+event.damage,0);
   const spellDirect=own.filter(event=>event.sourceKind==="explicit"&&event.damageType==="spell");
   const spellDotDamage=own.filter(event=>event.sourceKind==="dot"&&!procDots.has(`${name.toLowerCase()}\u001f${event.attack.toLowerCase()}`)).reduce((sum,event)=>sum+event.damage,0);
   const spellDotCount=directDots.filter(dot=>dot.casterName.toLowerCase()===name.toLowerCase()).length;
   const sources=[...new Map(own.map(event=>{const source=describeTrainingEventSource(encounter,event);return[source.label.toLowerCase(),source]})).values()];
   return {name,totalDamage,incomingDamage:received.reduce((sum,event)=>sum+event.damage,0),hitCount:own.length,dps:totalDamage/Math.max(1,combatSeconds),combatSeconds,contribution:0,procCount,procDirectDamage,procDotDamage,spellCount:spellDirect.length+spellDotCount,spellDirectDamage:spellDirect.reduce((sum,event)=>sum+event.damage,0),spellDotDamage,sources};
  }).sort((a,b)=>b.totalDamage-a.totalDamage||a.name.localeCompare(b.name));
  const incomingNames=new Map<string,string>();
  incoming.forEach(event=>incomingNames.set(event.target.toLowerCase(),event.target));
  const incomingTargets:DamageTarget[]=[...incomingNames].map(([key,name])=>{
   const received=incoming.filter(event=>event.target.toLowerCase()===key);
   return {
    name,
    totalDamage:received.reduce((sum,event)=>sum+event.damage,0),
    hitCount:received.length,
    maxHit:Math.max(0,...received.map(event=>event.damage)),
    firstDamageAt:received[0].happenedAt,
    lastDamageAt:received.at(-1)!.happenedAt,
   };
  });
  const totalDamage=events.reduce((sum,event)=>sum+event.damage,0);
  fighters.forEach(fighter=>fighter.contribution=fighter.totalDamage/Math.max(1,totalDamage)*100);
  return [{encounterId:encounter.id,mobName:encounter.mobName,startedAt:encounter.startedAt,currentAt:new Date(currentMs).toISOString(),totalDamage,elapsedSeconds,fighters,incomingTargets}];
 });
}