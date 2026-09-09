import type {ActiveDot,DamageEncounter,DamageEvent,DamageSpellMetric,DotTrainingEncounter,DotTrainingEvent} from "../../shared/contracts";

export type DamageSourceConfidence="exact"|"strong"|"candidate"|"unknown";
export interface DamageSourceExplanation { label:string; confidence:DamageSourceConfidence; evidence:string }

const PROC_ITEMS:Record<string,string>={
 "essence tap":"Essence Mace",
 "dawncall":"Great Spear of Dawn",
 "dawncaller":"Great Spear of Dawn",
};
const clean=(values:(string|undefined)[])=>[...new Set(values.map(value=>value?.trim()).filter((value):value is string=>Boolean(value)))];
const knownProcItem=(spellName:string)=>PROC_ITEMS[spellName.trim().toLowerCase()];
const damageMode=(direct:number,dot:number)=>direct>0&&dot>0?"DD + DoT":dot>0?"DoT":direct>0?"DD":"effect";

export function describeProcSpell(spellName:string,directDamage:number,dotDamage:number):DamageSourceExplanation{
 const item=knownProcItem(spellName),mode=damageMode(directDamage,dotDamage);
 return item
  ?{label:`${item} / proc ${mode} / ${spellName}`,confidence:"strong",evidence:"Known weapon-to-proc-spell association; combat adjacency identified the caster."}
  :{label:`Unknown weapon / proc ${mode} / ${spellName}`,confidence:"candidate",evidence:"The landing and adjacent attack prove a weapon proc, but neither the log nor the known mapping catalog identifies the weapon."};
}

export function describeSpellMetric(metric:DamageSpellMetric):DamageSourceExplanation[]{
 const rows:DamageSourceExplanation[]=[];
 if(metric.procCount>0)rows.push(describeProcSpell(metric.spellName,metric.directProcDamage,metric.procDotDamage));
 const castDot=Math.max(0,metric.dotDamage-metric.procDotDamage);
 if(castDot>0)rows.push({label:`Spell or item cast / DoT / ${metric.spellName}`,confidence:"exact",evidence:"The catalog landing matched this spell; its attribution method is available on the DoT application when active."});
 return rows;
}

function dotSource(spellName:string,dot:Pick<ActiveDot,"attributionMethod">|undefined):DamageSourceExplanation{
 if(dot?.attributionMethod==="proc")return describeProcSpell(spellName,0,1);
 if(dot?.attributionMethod==="direct_cast")return{label:`Direct cast / DoT / ${spellName}`,confidence:"exact",evidence:"An exact You begin casting clue preceded the catalog landing."};
 if(dot?.attributionMethod==="item_glow")return{label:`Item activation / DoT / ${spellName}`,confidence:"exact",evidence:"An item-glow clue identified the caster, but the current schema does not retain the item's name."};
 return{label:`Unconfirmed cast source / DoT / ${spellName}`,confidence:"unknown",evidence:"The catalog identified the spell, but the log did not provide a reliable caster-source clue."};
}

export function describeActiveDot(dot:ActiveDot):DamageSourceExplanation{return dotSource(dot.spellName,dot)}

export function describeDamageEvent(event:DamageEvent,encounter?:DamageEncounter):DamageSourceExplanation{
 const weapons=clean([event.primaryWeapon,event.secondaryWeapon,...(encounter?.weapons||[])]);
 if(event.source==="proc"){
  const known=describeProcSpell(event.attack,event.damage,0);
  if(known.confidence==="strong")return known;
  return weapons.length?{label:`Last known weapons (snapshot may be stale): ${weapons.join(" or ")} / proc DD / ${event.attack}`,confidence:"candidate",evidence:"The proc is confirmed. These are candidates from the most recent inventory snapshot and may no longer have been equipped; the log does not identify the triggering hand."}:known;
 }
 if(event.source==="dot"){
  const dot=encounter?.activeDots?.find(value=>value.spellName.toLowerCase()===event.attack.toLowerCase()&&value.casterName.toLowerCase()===event.attacker.toLowerCase());
  const metric=encounter?.spellMetrics?.find(value=>value.spellName.toLowerCase()===event.attack.toLowerCase()&&value.playerName.toLowerCase()===event.attacker.toLowerCase());
  if(dot)return dotSource(event.attack,dot);
  if(metric&&metric.procDotDamage>0)return describeProcSpell(event.attack,0,event.damage);
  return dotSource(event.attack,undefined);
 }
 if(event.damageType==="spell")return{label:`Logged spell damage / DD / ${event.attack==="non-melee"?"spell name unavailable":event.attack}`,confidence:event.attack==="non-melee"?"unknown":"exact",evidence:event.attack==="non-melee"?"EverQuest logged the damage amount but omitted the spell name.":"The combat log named the damaging spell."};
 return weapons.length?{label:`Last known weapons (snapshot may be stale): ${weapons.join(" or ")} / melee / ${event.attack}`,confidence:"candidate",evidence:"The most recent inventory snapshot supplies only stale-capable candidates; the log proves neither that they remained equipped nor which hand caused this hit."}:{label:`Weapon unavailable / melee / ${event.attack}`,confidence:"unknown",evidence:"No weapon snapshot was available; no weapon is inferred."};
}

export function playerDamageSourceSummary(encounter:DamageEncounter,playerName:string,events:DamageEvent[]=[]):DamageSourceExplanation[]{
 const key=playerName.toLowerCase(),rows:DamageSourceExplanation[]=[];
 for(const metric of encounter.spellMetrics?.filter(value=>value.playerName.toLowerCase()===key)||[])rows.push(...describeSpellMetric(metric));
 for(const event of events.filter(value=>value.attacker.toLowerCase()===key&&value.source==="explicit"))rows.push(describeDamageEvent(event,encounter));
 const unique=new Map(rows.map(row=>[row.label.toLowerCase(),row]));
 return [...unique.values()];
}

export function describeTrainingEventSource(encounter:DotTrainingEncounter,event:DotTrainingEvent):DamageSourceExplanation{
 if(event.sourceKind==="proc")return describeProcSpell(event.attack,event.damage,0);
 if(event.sourceKind==="dot"){
  const dot=encounter.dots.find(value=>value.spellName.toLowerCase()===event.attack.toLowerCase()&&value.casterName.toLowerCase()===event.attacker.toLowerCase());
  return dotSource(event.attack,dot);
 }
 if(event.damageType==="spell")return{label:`Logged spell damage / DD / ${event.attack==="non-melee"?"spell name unavailable":event.attack}`,confidence:event.attack==="non-melee"?"unknown":"exact",evidence:"This is an explicit spell-damage event from the pasted log."};
 return{label:`Weapon unavailable / melee / ${event.attack}`,confidence:"unknown",evidence:"Training input has no inventory weapon snapshot, so no weapon is inferred."};
}