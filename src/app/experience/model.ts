import type {ExperienceGainEvent,ExperienceLevelEvent,ExperienceObservation} from "../../shared/contracts";

const time=(value:string)=>new Date(value.includes("T")?value:value.replace(" ","T")).getTime();
const MAX_ACTIVE_GAP_MS=10*60*1000;

export interface HistoricalLevelPace {
 fromLevel:number;
 toLevel:number;
 startedAt:string;
 endedAt:string;
 gainCount:number;
 activeHours:number;
 xpPerHour?:number;
 xpPerGain?:number;
}

export interface CurrentLevelWindow {
 level?:number;
 startedAt?:string;
 observations:ExperienceObservation[];
 gainEvents:ExperienceGainEvent[];
}

export interface ExperienceContextSample {
 event:ExperienceGainEvent;
 xp:number;
}

export interface ExperienceContextSummary {
 label:string;
 gains:number;
 correlatedGains:number;
 xp:number;
 xpPerGain?:number;
 perHour?:number;
 detail?:string;
}

export function summarizeExperienceContexts(samples:ExperienceContextSample[],events:ExperienceGainEvent[],label:(event:ExperienceGainEvent)=>string,detail?:(event:ExperienceGainEvent)=>string|undefined){
 const groups=new Map<string,{gains:number;correlatedGains:number;xp:number;times:string[];detail?:string}>();
 events.forEach(event=>{
  const key=label(event)||"Unknown",row=groups.get(key)||{gains:0,correlatedGains:0,xp:0,times:[],detail:detail?.(event)};
  row.gains++;row.detail||=detail?.(event);groups.set(key,row);
 });
 samples.forEach(({event,xp})=>{
  const key=label(event)||"Unknown",row=groups.get(key)||{gains:1,correlatedGains:0,xp:0,times:[],detail:detail?.(event)};
  row.correlatedGains++;row.xp+=xp;row.times.push(event.happenedAt);row.detail||=detail?.(event);groups.set(key,row);
 });
 return [...groups].map(([key,row]):ExperienceContextSummary=>{
  const hours=activeHoursForTimes(row.times);
  return{label:key,gains:row.gains,correlatedGains:row.correlatedGains,xp:row.xp,xpPerGain:row.correlatedGains?row.xp/row.correlatedGains:undefined,perHour:row.correlatedGains>1&&hours>.01?row.xp/hours:undefined,detail:row.detail};
 }).sort((a,b)=>(b.perHour??b.xpPerGain??-1)-(a.perHour??a.xpPerGain??-1)||b.gains-a.gains||a.label.localeCompare(b.label));
}

export function currentLevelWindow(character:string,observations:ExperienceObservation[],gainEvents:ExperienceGainEvent[],levelEvents:ExperienceLevelEvent[]):CurrentLevelWindow{
 const characterObservations=observations.filter(row=>row.character===character).sort((a,b)=>time(b.happenedAt)-time(a.happenedAt));
 const latestTransition=levelEvents.filter(row=>row.character===character).sort((a,b)=>time(b.happenedAt)-time(a.happenedAt))[0];
 const level=latestTransition?.level??characterObservations[0]?.level;
 const transitionTime=latestTransition?time(latestTransition.happenedAt):undefined;
 let currentObservations=characterObservations.filter(row=>(transitionTime==null||time(row.happenedAt)>transitionTime)&&(level==null||row.level==null||row.level===level));
 if(transitionTime==null&&level!=null)currentObservations=characterObservations.filter(row=>row.level==null||row.level===level);
 const inferredStart=currentObservations.length?Math.min(...currentObservations.map(row=>time(row.happenedAt))):undefined;
 const cutoff=transitionTime??inferredStart;
 const currentGainEvents=gainEvents
  .filter(row=>row.character===character&&(cutoff==null||time(row.happenedAt)>cutoff))
  .sort((a,b)=>time(b.happenedAt)-time(a.happenedAt));
 return{level,startedAt:latestTransition?.happenedAt??(inferredStart==null?undefined:new Date(inferredStart).toISOString()),observations:currentObservations,gainEvents:currentGainEvents};
}

function activeElapsedMilliseconds(values:number[]){
 const ordered=[...values].sort((a,b)=>a-b);
 let total=0;
 for(let index=1;index<ordered.length;index++){
  const gap=Math.max(0,ordered[index]-ordered[index-1]);
  if(gap<=MAX_ACTIVE_GAP_MS)total+=gap;
 }
 return total;
}

function activeMilliseconds(start:number,end:number,events:ExperienceGainEvent[]){
 return activeElapsedMilliseconds([start,...events.map(event=>time(event.happenedAt)),end]);
}

export function activeHoursForTimes(values:string[]){
 return activeElapsedMilliseconds(values.map(time))/3_600_000;
}

function observationGain(previous:ExperienceObservation,current:ExperienceObservation){
 if(current.level===previous.level&&current.percent>=previous.percent)return current.percent-previous.percent;
 if(current.level!=null&&previous.level!=null&&current.level===previous.level+1)return 100-previous.percent+current.percent;
 return undefined;
}

export function visualExperienceRate(rows:ExperienceObservation[],now:number){
 const ordered=[...rows]
  .filter(row=>now-time(row.happenedAt)<=60*60*1000)
  .sort((a,b)=>time(a.happenedAt)-time(b.happenedAt));
 let gained=0,activeMs=0;
 for(let index=1;index<ordered.length;index++){
  const previous=ordered[index-1],current=ordered[index];
  const gap=time(current.happenedAt)-time(previous.happenedAt);
  if(gap<=0||gap>MAX_ACTIVE_GAP_MS)continue;
  const delta=observationGain(previous,current);
  if(delta==null||delta<0||delta>100)continue;
  activeMs+=gap;
  gained+=delta;
 }
 return activeMs>=60_000?gained/(activeMs/3_600_000):undefined;
}

export function historicalLevelPaces(levelEvents:ExperienceLevelEvent[],gainEvents:ExperienceGainEvent[]){
 const levels=levelEvents.filter(event=>event.direction==="gained").sort((a,b)=>time(a.happenedAt)-time(b.happenedAt)),rows:HistoricalLevelPace[]=[];
 for(let index=1;index<levels.length;index++){
  const previous=levels[index-1],current=levels[index];
  if(current.level!==previous.level+1)continue;
  const start=time(previous.happenedAt),end=time(current.happenedAt);
  const gains=gainEvents.filter(event=>{const value=time(event.happenedAt);return value>start&&value<=end});
  const activeMs=activeMilliseconds(start,end,gains),activeHours=activeMs/3600000;
  rows.push({fromLevel:previous.level,toLevel:current.level,startedAt:previous.happenedAt,endedAt:current.happenedAt,gainCount:gains.length,activeHours,xpPerHour:gains.length&&activeHours>0?100/activeHours:undefined,xpPerGain:gains.length?100/gains.length:undefined});
 }
 return rows.reverse();
}

export function combinedHistoricalRate(rows:HistoricalLevelPace[]){
 const usable=rows.filter(row=>row.xpPerHour!=null&&row.activeHours>0);
 const hours=usable.reduce((total,row)=>total+row.activeHours,0);
 return hours?usable.length*100/hours:undefined;
}
