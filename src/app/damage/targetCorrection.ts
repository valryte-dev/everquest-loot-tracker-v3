import type {DamageEncounter} from "../../shared/contracts";

export interface TargetCorrectionCandidate {
 id:number;
 mobName:string;
 myDamage:number;
 groupDamage:number;
}

const normalized=(value:string)=>value.trim().toLocaleLowerCase();

export function buildTargetCorrectionCandidates(current:DamageEncounter,encounters:DamageEncounter[]):TargetCorrectionCandidate[]{
 const character=normalized(current.character),currentTarget=normalized(current.mobName);
 const unique=new Map<string,TargetCorrectionCandidate>();
 encounters.forEach(encounter=>{
  const mobName=encounter.mobName.trim(),key=normalized(mobName);
  if(!mobName||key===currentTarget||normalized(encounter.character)!==character)return;
  const myDamage=encounter.players.find(player=>normalized(player.name)===character)?.totalDamage||0;
  const candidate={id:encounter.id,mobName,myDamage,groupDamage:encounter.totalDamage};
  const existing=unique.get(key);
  if(!existing||candidate.myDamage>existing.myDamage||candidate.myDamage===existing.myDamage&&candidate.groupDamage>existing.groupDamage)unique.set(key,candidate);
 });
 return [...unique.values()].sort((a,b)=>b.myDamage-a.myDamage||b.groupDamage-a.groupDamage||a.mobName.localeCompare(b.mobName));
}
