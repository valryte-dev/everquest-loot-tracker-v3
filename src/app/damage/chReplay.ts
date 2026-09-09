import type {ClericHealCall,ClericHealReplayCall,ClericHealReplayFile,DamageEncounter} from "../../shared/contracts";
import {buildClericChainTimeline,type TimedClericHealCall} from "./model";

export interface ClericChainClericStats {
 name:string;
 callCount:number;
 intervalCount:number;
 averageRotationSeconds:number;
 standardDeviationSeconds:number;
 shortestRotationSeconds:number;
 longestRotationSeconds:number;
}

export interface ClericChainEncounter {
 id:string;
 character:string;
 targetMob:string;
 startedAt:string;
 endedAt:string;
 durationSeconds:number;
 callCount:number;
 healerCount:number;
 averageGapSeconds:number;
 longestGapSeconds:number;
 status:"active"|"concluded";
 clericStats:ClericChainClericStats[];
 calls:TimedClericHealCall[];
}

const stamp=(value:string)=>{const normalized=value.includes("T")?value:value.replace(" ","T");const parsed=Date.parse(normalized);return Number.isFinite(parsed)?parsed:0};
const encounterMob=(calls:TimedClericHealCall[],encounters:DamageEncounter[])=>{
 const start=stamp(calls[0].happenedAt),end=stamp(calls.at(-1)!.happenedAt),character=calls[0].character.toLowerCase();
 const relevant=encounters.filter(row=>row.character.toLowerCase()===character);
 const overlapping=relevant.filter(row=>stamp(row.startedAt)<=end+15000&&stamp(row.endedAt||row.lastDamageAt)>=start-15000);
 const candidates=overlapping.length?overlapping:relevant.filter(row=>stamp(row.startedAt)<=end).slice(0,20);
 return [...candidates].sort((a,b)=>Math.abs(stamp(a.lastDamageAt)-end)-Math.abs(stamp(b.lastDamageAt)-end))[0]?.mobName||"Unknown mob";
};

export function calculateClericChainStats(calls:TimedClericHealCall[]):ClericChainClericStats[]{
 const grouped=new Map<string,{name:string;calls:number;intervals:number[]}>();
 calls.forEach(call=>{const key=call.clericName.toLowerCase(),row=grouped.get(key)||{name:call.clericName,calls:0,intervals:[]};row.calls++;if(call.clericGapSeconds!==undefined)row.intervals.push(call.clericGapSeconds);grouped.set(key,row)});
 return [...grouped.values()].map(row=>{
  const average=row.intervals.length?row.intervals.reduce((sum,value)=>sum+value,0)/row.intervals.length:0;
  const deviation=row.intervals.length?Math.sqrt(row.intervals.reduce((sum,value)=>sum+(value-average)**2,0)/row.intervals.length):0;
  return {name:row.name,callCount:row.calls,intervalCount:row.intervals.length,averageRotationSeconds:average,standardDeviationSeconds:deviation,shortestRotationSeconds:row.intervals.length?Math.min(...row.intervals):0,longestRotationSeconds:Math.max(0,...row.intervals)};
 }).sort((a,b)=>b.callCount-a.callCount||a.name.localeCompare(b.name));
}

export function buildClericChainEncounters(calls:ClericHealCall[],damageEncounters:DamageEncounter[],nowMs=Date.now()):ClericChainEncounter[]{
 const boundaries=damageEncounters.filter(row=>row.outcome==="slain"&&row.endedAt).map(row=>stamp(row.endedAt!));
 const timeline=buildClericChainTimeline(calls,15,boundaries);
 const groups=new Map<string,TimedClericHealCall[]>();
 timeline.forEach(call=>{const key=`${call.character.toLowerCase()}|${call.session}`;const group=groups.get(key)||[];group.push(call);groups.set(key,group)});
 const built=[...groups.values()].map(group=>{
  const ordered=[...group].sort((a,b)=>stamp(a.happenedAt)-stamp(b.happenedAt)||a.id-b.id);
  const gaps=ordered.flatMap(call=>call.gapSeconds===undefined?[]:[call.gapSeconds]);
  return {
   id:`${ordered[0].character}:${ordered[0].id}:${ordered.at(-1)!.id}`,
   character:ordered[0].character,targetMob:encounterMob(ordered,damageEncounters),startedAt:ordered[0].happenedAt,endedAt:ordered.at(-1)!.happenedAt,
   durationSeconds:Math.max(0,(stamp(ordered.at(-1)!.happenedAt)-stamp(ordered[0].happenedAt))/1000),callCount:ordered.length,
   healerCount:new Set(ordered.map(call=>call.clericName.toLowerCase())).size,
   averageGapSeconds:gaps.length?gaps.reduce((sum,gap)=>sum+gap,0)/gaps.length:0,longestGapSeconds:Math.max(0,...gaps),
   status:"active" as const,clericStats:calculateClericChainStats(ordered),calls:ordered,
  };
 });
 return built.map(row=>{
  const ended=stamp(row.endedAt),hasLater=built.some(other=>other.character.toLowerCase()===row.character.toLowerCase()&&stamp(other.startedAt)>stamp(row.startedAt));
  const slain=damageEncounters.some(fight=>fight.character.toLowerCase()===row.character.toLowerCase()&&fight.outcome==="slain"&&!!fight.endedAt&&stamp(fight.endedAt)>=ended&&stamp(fight.endedAt)<=ended+15000);
  return {...row,status:hasLater||slain||nowMs-ended>=15000?"concluded" as const:"active" as const};
 }).sort((a,b)=>stamp(b.startedAt)-stamp(a.startedAt));
}

export function refreshClericChainEncounterStatuses(rows:ClericChainEncounter[],nowMs=Date.now()):ClericChainEncounter[]{
 return rows.map(row=>row.status==="active"&&nowMs-stamp(row.endedAt)>=15000?{...row,status:"concluded"}:row);
}

export const replayCalls=(encounter:ClericChainEncounter):ClericHealReplayCall[]=>encounter.calls.map(call=>({...call}));

export function encounterFromReplay(replay:ClericHealReplayFile,path:string):ClericChainEncounter{
 const calls:TimedClericHealCall[]=replay.calls.map(call=>({...call,session:0,gapSeconds:typeof call.gapSeconds==="number"&&Number.isFinite(call.gapSeconds)?call.gapSeconds:undefined,clericGapSeconds:typeof call.clericGapSeconds==="number"&&Number.isFinite(call.clericGapSeconds)?call.clericGapSeconds:undefined}));
 return {id:path||replay.title,character:replay.character,targetMob:replay.targetMob,startedAt:replay.summary.startedAt,endedAt:replay.summary.endedAt,durationSeconds:replay.summary.durationSeconds,callCount:replay.summary.callCount,healerCount:replay.summary.healerCount,averageGapSeconds:replay.summary.averageGapSeconds,longestGapSeconds:replay.summary.longestGapSeconds,status:"concluded",clericStats:calculateClericChainStats(calls),calls};
}