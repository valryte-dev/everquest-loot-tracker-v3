import type {DamageEvent,DamageParticipant} from "../../shared/contracts";

const stamp=(value:string)=>Date.parse(value.includes("T")?value:value.replace(" ","T"));

export interface MeterPlayer extends DamageParticipant {
 rank:number;
 contribution:number;
 dps:number;
}

export interface MeterTrendPoint {
 second:number;
 totals:Record<string,number>;
}

export function formatCombatClock(seconds:number):string{
 const value=Math.max(0,Math.floor(seconds));
 return [Math.floor(value/3600),Math.floor(value%3600/60),value%60].map(part=>String(part).padStart(2,"0")).join(":");
}

export function participantCombatSeconds(player:DamageParticipant,encounterEndMs:number):number{
 return Math.max(0,Math.floor((encounterEndMs-stamp(player.firstDamageAt))/1000));
}

export function rankMeterPlayers(players:DamageParticipant[],totalDamage:number,durationSeconds:number):MeterPlayer[]{
 return [...players]
  .sort((a,b)=>b.totalDamage-a.totalDamage||a.name.localeCompare(b.name))
  .map((player,index)=>({
   ...player,
   rank:index+1,
   contribution:player.totalDamage/Math.max(1,totalDamage)*100,
   dps:player.totalDamage/Math.max(1,durationSeconds),
  }));
}

export function buildMeterTrend(events:DamageEvent[],startedAt:string,names:string[],maxPoints=60):MeterTrendPoint[]{
 const canonical=new Map(names.map(name=>[name.toLowerCase(),name]));
 const bySecond=new Map<number,Map<string,number>>();
 for(const event of [...events].sort((a,b)=>stamp(a.happenedAt)-stamp(b.happenedAt)||a.id-b.id)){
  const name=canonical.get(event.attacker.toLowerCase());
  if(!name)continue;
  const second=Math.max(0,Math.floor((stamp(event.happenedAt)-stamp(startedAt))/1000));
  const bucket=bySecond.get(second)||new Map<string,number>();
  bucket.set(name,(bucket.get(name)||0)+event.damage);
  bySecond.set(second,bucket);
 }
 const totals:Record<string,number>=Object.fromEntries(names.map(name=>[name,0]));
 const points:MeterTrendPoint[]=[{second:0,totals:{...totals}}];
 for(const [second,damage] of [...bySecond].sort((a,b)=>a[0]-b[0])){
  for(const [name,amount] of damage)totals[name]=(totals[name]||0)+amount;
  if(second===0)points[0]={second,totals:{...totals}};
  else points.push({second,totals:{...totals}});
 }
 if(points.length<=maxPoints)return points;
 const sampled:MeterTrendPoint[]=[];
 for(let index=0;index<maxPoints;index++){
  const source=Math.round(index*(points.length-1)/(maxPoints-1));
  if(!sampled.length||sampled.at(-1)!==points[source])sampled.push(points[source]);
 }
 return sampled;
}
