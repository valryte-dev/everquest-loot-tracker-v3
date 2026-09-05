import type {DamageEncounter,DamageEvent} from "../../shared/contracts";

export interface LiveDpsPoint { second:number; group:number; me:number }
export interface DamageBurstPoint { second:number; group:number; me:number }

const timestamp=(value:string)=>Date.parse(value.includes("T")?value:value.replace(" ","T"));

export function buildLiveDpsSeries(
 events:DamageEvent[],
 character:string,
 startedAt:string,
 endSecond:number,
 horizon=30,
 rollingWindow=5,
):LiveDpsPoint[]{
 const end=Math.max(0,Math.floor(endSecond)),start=Math.max(0,end-horizon),origin=timestamp(startedAt);
 const timed=events.map(event=>({event,second:Math.max(0,(timestamp(event.happenedAt)-origin)/1000)}));
 return Array.from({length:end-start+1},(_,index)=>{
  const second=start+index,lower=second-rollingWindow,windowEvents=timed.filter(value=>value.second>lower&&value.second<=second);
  const divisor=Math.max(1,Math.min(rollingWindow,second+1));
  const group=windowEvents.reduce((sum,value)=>sum+value.event.damage,0)/divisor;
  const me=windowEvents.filter(value=>value.event.attacker.toLowerCase()===character.toLowerCase()).reduce((sum,value)=>sum+value.event.damage,0)/divisor;
  return{second,group,me};
 });
}

export function dpsMomentum(points:LiveDpsPoint[],key:"group"|"me"):"rising"|"steady"|"falling"{
 if(points.length<2)return"steady";
 const current=points[points.length-1][key],prior=points[Math.max(0,points.length-6)][key],threshold=Math.max(1,prior*.08);
 return current>prior+threshold?"rising":current<prior-threshold?"falling":"steady";
}

export function buildDamageBurstSeries(
 events:DamageEvent[],
 character:string,
 startedAt:string,
 endSecond:number,
 horizon=30,
):DamageBurstPoint[]{
 const end=Math.max(0,Math.floor(endSecond)),start=Math.max(0,end-horizon+1),origin=timestamp(startedAt);
 const points=Array.from({length:end-start+1},(_,index)=>({second:start+index,group:0,me:0}));
 for(const event of events){
  const second=Math.floor(Math.max(0,(timestamp(event.happenedAt)-origin)/1000));
  if(second<start||second>end)continue;
  const point=points[second-start];
  point.group+=event.damage;
  if(event.attacker.toLowerCase()===character.toLowerCase())point.me+=event.damage;
 }
 return points;
}

export function selectLiveEncounters(
 rows:DamageEncounter[],
 activeCharacter:string|undefined,
 seenAt:ReadonlyMap<number,number>,
 now:number,
 closed:number[],
 ttl=31000,
):DamageEncounter[]{
 return rows
  .filter(row=>(!activeCharacter||row.character.toLowerCase()===activeCharacter.toLowerCase())&&!closed.includes(row.id)&&now-(seenAt.get(row.id)||0)<=ttl)
  .sort((a,b)=>(seenAt.get(b.id)||0)-(seenAt.get(a.id)||0));
}

