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
