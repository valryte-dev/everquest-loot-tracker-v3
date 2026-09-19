import type {ClericHealCall,DamageEncounter,DamageEvent} from "../../shared/contracts";

export interface LiveDpsPoint { second:number; group:number; me:number }
export interface DamageBurstPoint { second:number; group:number; me:number }

const timestamp=(value:string)=>Date.parse(value.includes("T")?value:value.replace(" ","T"));

export function damageEncounterSnapshotChanged(
 pageRows:DamageEncounter[],
 liveRows:DamageEncounter[],
):boolean{
 const current=new Map(pageRows.map(row=>[row.id,row]));
 return liveRows.some(row=>{
  const page=current.get(row.id);
  return !page
   ||page.lastDamageAt!==row.lastDamageAt
   ||page.totalDamage!==row.totalDamage
   ||page.hitCount!==row.hitCount
   ||(page.incomingDamage||0)!==(row.incomingDamage||0)
   ||(page.incomingHitCount||0)!==(row.incomingHitCount||0)
   ||page.outcome!==row.outcome;
 });
}

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

export function sortLiveEncountersByPlayerTarget(
 rows:DamageEncounter[],
 activeCharacter?:string,
 preferredEncounterId?:number,
):DamageEncounter[]{
 const outgoingAt=(row:DamageEncounter)=>{
  const character=(activeCharacter||row.character).toLowerCase();
  const participant=row.players.find(player=>player.name.toLowerCase()===character);
  return participant?timestamp(participant.lastDamageAt):0;
 };
 return [...rows].sort((a,b)=>{
  if(a.id===preferredEncounterId&&b.id!==preferredEncounterId)return -1;
  if(b.id===preferredEncounterId&&a.id!==preferredEncounterId)return 1;
  const aOutgoing=outgoingAt(a),bOutgoing=outgoingAt(b);
  if(Boolean(aOutgoing)!==Boolean(bOutgoing))return bOutgoing?1:-1;
  return bOutgoing-aOutgoing||timestamp(b.lastDamageAt)-timestamp(a.lastDamageAt)||b.id-a.id;
 });
}

export function preferredDamageTargetId(settings:Record<string,string>,activeCharacter?:string):number|undefined{
 if(!activeCharacter||settings.damage_target_character?.toLowerCase()!==activeCharacter.toLowerCase())return undefined;
 const id=Number(settings.damage_target_encounter_id);
 return Number.isSafeInteger(id)&&id>0?id:undefined;
}

export interface TimedClericHealCall extends ClericHealCall {
 session:number;
 gapSeconds?:number;
 clericGapSeconds?:number;
}

export function buildClericChainTimeline(
 calls:ClericHealCall[],
 sessionGapSeconds=15,
 hardBoundaries:number[]=[],
):TimedClericHealCall[]{
 const ordered=[...calls].sort((a,b)=>timestamp(a.happenedAt)-timestamp(b.happenedAt)||a.id-b.id);
 let session=0,lastOverall:number|undefined,lastCharacter:string|undefined;
 const lastByCleric=new Map<string,number>();
 return ordered.map(call=>{
  const current=timestamp(call.happenedAt),character=call.character.toLowerCase();
  const characterChanged=lastCharacter!==undefined&&lastCharacter!==character;
  const crossedBoundary=lastOverall!==undefined&&hardBoundaries.some(boundary=>boundary>lastOverall!&&boundary<=current);
  const rawGap=lastOverall===undefined||characterChanged?undefined:Math.max(0,(current-lastOverall)/1000);
  if(characterChanged||crossedBoundary||(rawGap!==undefined&&rawGap>sessionGapSeconds)){
   session++;
   lastByCleric.clear();
  }
  const clericKey=call.clericName.toLowerCase(),lastCleric=lastByCleric.get(clericKey);
  const result:TimedClericHealCall={
   ...call,
   session,
   gapSeconds:!crossedBoundary&&rawGap!==undefined&&rawGap<=sessionGapSeconds?rawGap:undefined,
   clericGapSeconds:lastCleric===undefined?undefined:Math.max(0,(current-lastCleric)/1000),
  };
  lastOverall=current;
  lastCharacter=character;
  lastByCleric.set(clericKey,current);
  return result;
 });
}

export function discordHealChainSummary(calls:TimedClericHealCall[],tankName?:string):string{
 if(!calls.length)return "**Complete Heal Chain Summary**\n_No active calls to summarize._";
 const ordered=[...calls].sort((a,b)=>timestamp(a.happenedAt)-timestamp(b.happenedAt)||a.id-b.id);
 const gaps=ordered.flatMap(call=>call.gapSeconds===undefined?[]:[call.gapSeconds]);
 const average=gaps.length?gaps.reduce((sum,gap)=>sum+gap,0)/gaps.length:0;
 const sortedGaps=[...gaps].sort((a,b)=>a-b);
 const median=gaps.length?(sortedGaps[Math.floor((gaps.length-1)/2)]+sortedGaps[Math.floor(gaps.length/2)])/2:0;
 const deviation=gaps.length?Math.sqrt(gaps.reduce((sum,gap)=>sum+(gap-average)**2,0)/gaps.length):0;
 const elapsedSeconds=Math.max(0,(timestamp(ordered.at(-1)!.happenedAt)-timestamp(ordered[0].happenedAt))/1000);
 const duration=elapsedSeconds>=60?`${Math.floor(elapsedSeconds/60)}m ${Math.round(elapsedSeconds%60)}s`:`${Math.round(elapsedSeconds)}s`;
 const healerCount=new Set(ordered.map(call=>call.clericName.toLowerCase())).size;
 const recent=ordered.slice(-12).map((call,index)=>`${index===0&&ordered.length>12?"... -> ":""}#${String(call.callNumber).padStart(3,"0")}${call.gapSeconds===undefined?" (start)":` (+${call.gapSeconds.toFixed(1)}s)`}`).join(" -> ");
 const range=gaps.length?`${Math.min(...gaps).toFixed(1)}s - ${Math.max(...gaps).toFixed(1)}s`:"Awaiting a second call";
 return [
  "**Complete Heal Chain Summary**",
  `**Tank:** ${tankName?.trim()||"Unknown tank"}`,
  `**${ordered.length} calls - ${healerCount} anonymous healer${healerCount===1?"":"s"} - ${duration} elapsed**`,
  `- Average gap: **${average.toFixed(1)}s**`,
  `- Median gap: **${median.toFixed(1)}s**`,
  `- Gap range: **${range}**`,
  `- Consistency: **+/- ${deviation.toFixed(1)}s**`,
  `- Gaps over 12s: **${gaps.filter(gap=>gap>12).length}**`,
  "",
  "**Recent cadence**",
  `\`${recent}\``,
 ].join("\n");
}
export function latestHealChainBoundary(
 _encounters:DamageEncounter[],
 _character:string|undefined,
 manualClearedAt:string|undefined,
):number{
 const manual=manualClearedAt?timestamp(manualClearedAt):Number.NaN;
 // A character can observe many simultaneous fights during a raid. Treating
 // every slain mob as the end of the CH chain hides valid calls whenever
 // unrelated trash dies. The live chain already has a 15-second inactivity
 // boundary, while this value represents only an explicit user clear.
 return Number.isFinite(manual)?manual:0;
}
