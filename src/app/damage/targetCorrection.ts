import type {DamageEncounter} from "../../shared/contracts";

export interface TargetCorrectionCandidate {
 id:number;
 mobName:string;
 myDamage:number;
 groupDamage:number;
 source:"encounter"|"fighter";
}

const normalized=(value:string)=>value.trim().toLocaleLowerCase();

export function buildTargetCorrectionCandidates(current:DamageEncounter,encounters:DamageEncounter[]):TargetCorrectionCandidate[]{
 const character=normalized(current.character),currentTarget=normalized(current.mobName);
 const unique=new Map<string,TargetCorrectionCandidate>();
 const add=(candidate:TargetCorrectionCandidate)=>{
  const key=normalized(candidate.mobName),existing=unique.get(key);
  if(!key||key===currentTarget||key===character)return;
  const replacesFighter=existing?.source==="fighter"&&candidate.source==="encounter";
  const improvesSameSource=existing?.source===candidate.source&&(candidate.myDamage>existing.myDamage||candidate.myDamage===existing.myDamage&&candidate.groupDamage>existing.groupDamage);
  if(!existing||replacesFighter||improvesSameSource)unique.set(key,candidate);
 };
 encounters.forEach(encounter=>{
  const mobName=encounter.mobName.trim(),key=normalized(mobName);
  if(!mobName||key===currentTarget||normalized(encounter.character)!==character)return;
  const myDamage=encounter.players.find(player=>normalized(player.name)===character)?.totalDamage||0;
  add({id:encounter.id,mobName,myDamage,groupDamage:encounter.totalDamage,source:"encounter"});
 });
 current.players.forEach(player=>add({
  id:current.id,
  mobName:player.name.trim(),
  myDamage:0,
  groupDamage:player.totalDamage,
  source:"fighter",
 }));
 return [...unique.values()].sort((a,b)=>b.myDamage-a.myDamage||b.groupDamage-a.groupDamage||a.mobName.localeCompare(b.mobName));
}
