import type {DamageEncounter,DamageEvent,DamageParticipant,DamageSpellMetric,DamageTarget} from "../../shared/contracts";

const stamp=(value:string)=>Date.parse(value.includes("T")?value:value.replace(" ","T"));

export interface MeterPlayer extends DamageParticipant {
 rank:number;
 contribution:number;
 dps:number;
}

export interface IncomingTarget extends DamageTarget {
 rank:number;
 contribution:number;
}

export interface SpellMetricTotals {
 procCount:number;
 directProcDamage:number;
 dotDamage:number;
 dotTickCount:number;
 procDotDamage:number;
 totalProcDamage:number;
}

export interface PlayerEffectTotals {
 procCount:number;
 procDirectDamage:number;
 procDotDamage:number;
 spellCount:number;
 spellDirectDamage:number;
 spellDotDamage:number;
}
export interface ProcLeader {
 name:string;
 rank:number;
 procCount:number;
 directDamage:number;
 dotDamage:number;
 totalDamage:number;
 contribution:number;
}

export interface DamageComposition {
 melee:number;
 direct:number;
 dot:number;
 total:number;
 meleePercent:number;
 directPercent:number;
 dotPercent:number;
}
export interface MobPerformanceRanking {
 mob:string;
 damage:number;
 durationSeconds:number;
 count:number;
 dps:number;
}
export interface MobPerformanceRankings {
 byDamage:MobPerformanceRanking[];
 byDps:MobPerformanceRanking[];
}

export function playerSpellMetrics(encounter:DamageEncounter,playerName:string):DamageSpellMetric[]{
 const wanted=playerName.trim().toLowerCase();
 return (encounter.spellMetrics||[]).filter(metric=>metric.playerName.trim().toLowerCase()===wanted);
}

export function summarizeSpellMetrics(metrics:DamageSpellMetric[]):SpellMetricTotals{
 return metrics.reduce((total,metric)=>({
  procCount:total.procCount+metric.procCount,
  directProcDamage:total.directProcDamage+metric.directProcDamage,
  dotDamage:total.dotDamage+metric.dotDamage,
  dotTickCount:total.dotTickCount+metric.dotTickCount,
  procDotDamage:total.procDotDamage+metric.procDotDamage,
  totalProcDamage:total.totalProcDamage+metric.totalProcDamage,
 }),{procCount:0,directProcDamage:0,dotDamage:0,dotTickCount:0,procDotDamage:0,totalProcDamage:0});
}

export function rankProccers(metrics:DamageSpellMetric[]):ProcLeader[]{
 const grouped=new Map<string,{name:string;procCount:number;directDamage:number;dotDamage:number}>();
 for(const metric of metrics){
  if(metric.procCount<=0&&metric.totalProcDamage<=0)continue;
  const key=metric.playerName.trim().toLowerCase();
  const current=grouped.get(key)||{name:metric.playerName,procCount:0,directDamage:0,dotDamage:0};
  current.procCount+=metric.procCount;
  current.directDamage+=metric.directProcDamage;
  current.dotDamage+=metric.procDotDamage;
  grouped.set(key,current);
 }
 const ranked=[...grouped.values()].sort((a,b)=>(b.directDamage+b.dotDamage)-(a.directDamage+a.dotDamage)||b.procCount-a.procCount||a.name.localeCompare(b.name));
 const total=ranked.reduce((sum,row)=>sum+row.directDamage+row.dotDamage,0);
 return ranked.map((row,index)=>({...row,rank:index+1,totalDamage:row.directDamage+row.dotDamage,contribution:(row.directDamage+row.dotDamage)/Math.max(1,total)*100}));
}
export function summarizePlayerEffects(encounter:DamageEncounter,events:DamageEvent[],playerName:string):PlayerEffectTotals{
 const metrics=playerSpellMetrics(encounter,playerName);
 const proc=summarizeSpellMetrics(metrics);
 const wanted=playerName.trim().toLowerCase();
 const explicitDirect=events.filter(event=>event.attacker.trim().toLowerCase()===wanted&&event.damageType==="spell"&&event.source!=="proc"&&event.source!=="dot");
 const spellDotDamage=Math.max(0,proc.dotDamage-proc.procDotDamage);
 const dotSpellCount=metrics.filter(metric=>metric.dotDamage>metric.procDotDamage).length;
 return {
  procCount:proc.procCount,
  procDirectDamage:proc.directProcDamage,
  procDotDamage:proc.procDotDamage,
  spellCount:explicitDirect.length+dotSpellCount,
  spellDirectDamage:explicitDirect.reduce((total,event)=>total+event.damage,0),
  spellDotDamage,
 };
}

export function damageComposition(encounter:Pick<DamageEncounter,"totalDamage"|"meleeDamage"|"spellDamage"|"dotDamage">):DamageComposition{
 const dot=Math.max(0,Math.min(encounter.dotDamage||0,encounter.spellDamage));
 const direct=Math.max(0,encounter.spellDamage-dot);
 const melee=Math.max(0,encounter.meleeDamage);
 const total=Math.max(1,encounter.totalDamage);
 return {melee,direct,dot,total:encounter.totalDamage,meleePercent:melee/total*100,directPercent:direct/total*100,dotPercent:dot/total*100};
}
export function rankMobsByPerformance(encounters:DamageEncounter[],limit=8):MobPerformanceRankings{
 const aggregate=(source:DamageEncounter[])=>{
  const grouped=new Map<string,{mob:string;damage:number;durationSeconds:number;count:number}>();
  for(const encounter of source){
   const key=encounter.mobName.trim().toLowerCase();
   const current=grouped.get(key)||{mob:encounter.mobName,damage:0,durationSeconds:0,count:0};
   current.damage+=encounter.totalDamage;
   current.durationSeconds+=Math.max(1,Math.round((stamp(encounter.lastDamageAt)-stamp(encounter.startedAt))/1000));
   current.count++;
   grouped.set(key,current);
  }
  return [...grouped.values()].map(row=>({...row,dps:row.damage/Math.max(1,row.durationSeconds)}));
 };
 const damageRows=aggregate(encounters);
 // Incoming-only encounter fragments can carry the player victim in mobName. For
 // DPS-by-mob, require evidence that the active log character damaged the target.
 const confirmedTargetRows=aggregate(encounters.filter(encounter=>
  encounter.players.some(player=>player.name.localeCompare(encounter.character,undefined,{sensitivity:"accent"})===0),
 ));
 return {
  byDamage:[...damageRows].sort((a,b)=>b.damage-a.damage||b.dps-a.dps||a.mob.localeCompare(b.mob)).slice(0,limit),
  byDps:[...confirmedTargetRows].sort((a,b)=>b.dps-a.dps||b.damage-a.damage||a.mob.localeCompare(b.mob)).slice(0,limit),
 };
}
export interface RollingDpsPoint {
 second:number;
 dps:Record<string,number>;
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

export function rankIncomingTargets(targets:DamageTarget[]):IncomingTarget[]{
 const total=targets.reduce((sum,target)=>sum+target.totalDamage,0);
 return [...targets]
  .sort((a,b)=>b.totalDamage-a.totalDamage||b.maxHit-a.maxHit||a.name.localeCompare(b.name))
  .map((target,index)=>({...target,rank:index+1,contribution:target.totalDamage/Math.max(1,total)*100}));
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
export function buildRollingDpsTrend(events:DamageEvent[],startedAt:string,names:string[],durationSeconds:number,windowSeconds=30,horizonSeconds=120):RollingDpsPoint[]{
 const canonical=new Map(names.map(name=>[name.toLowerCase(),name]));
 const bySecond=new Map<number,Map<string,number>>();
 const duration=Math.max(0,Math.floor(durationSeconds)),window=Math.max(1,Math.floor(windowSeconds));
 for(const event of events){
  const name=canonical.get(event.attacker.toLowerCase());
  if(!name)continue;
  const second=Math.max(0,Math.floor((stamp(event.happenedAt)-stamp(startedAt))/1000));
  if(second>duration)continue;
  const bucket=bySecond.get(second)||new Map<string,number>();
  bucket.set(name,(bucket.get(name)||0)+event.damage);
  bySecond.set(second,bucket);
 }
 const start=Math.max(0,duration-Math.max(1,Math.floor(horizonSeconds))),points:RollingDpsPoint[]=[];
 for(let second=start;second<=duration;second++){
  const totals:Record<string,number>=Object.fromEntries(names.map(name=>[name,0]));
  for(let cursor=Math.max(0,second-window+1);cursor<=second;cursor++){
   for(const [name,damage] of bySecond.get(cursor)||[])totals[name]=(totals[name]||0)+damage;
  }
  const divisor=Math.min(window,Math.max(1,second));
  points.push({second,dps:Object.fromEntries(names.map(name=>[name,(totals[name]||0)/divisor]))});
 }
 return points;
}
