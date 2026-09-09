import type {CSSProperties} from "react";
import type {DamageSpellMetric} from "../../shared/contracts";


export const damageBarColors=["#fb56a3","#a878fa","#57c7ff","#ffa357","#7bdfb2","#f37d8d"];
const number=(value:number)=>Math.round(value).toLocaleString();

export const formatFighterDuration=(value:number)=>{
 const total=Math.max(0,Math.floor(Number.isFinite(value)?value:0));
 const hours=Math.floor(total/3600);
 const minutes=Math.floor((total%3600)/60);
 const seconds=total%60;
 const parts:string[]=[];
 if(hours>0) parts.push(`${hours}h`);
 if(hours>0||minutes>0) parts.push(`${minutes}m`);
 parts.push(`${seconds}s`);
 return parts.join(" ");
};

export interface DamageFighterEffects {
 procCount:number;
 procDirectDamage:number;
 procDotDamage:number;
 spellCount:number;
 spellDirectDamage:number;
 spellDotDamage:number;
}

export interface DamageFighterBarRow {
 name:string;
 rank:number;
 totalDamage:number;
 dps:number;
 combatSeconds:number;
 contribution:number;
 contributionDelta?:number;
 incomingDamage:number;
 mine:boolean;
 shareDetail?:string;
 combatTimeTitle?:string;
 effects:DamageFighterEffects;
 effectsTitle?:string;
}

export const spellEffectTitle=(metrics:DamageSpellMetric[])=>metrics.length
 ?metrics.map(metric=>metric.spellName+": "+metric.procCount+" proc"+(metric.procCount===1?"":"s")+", "+number(metric.totalProcDamage)+" proc damage ("+number(metric.procDotDamage)+" DoT / "+number(metric.directProcDamage)+" direct), "+number(metric.dotDamage)+" total DoT").join("\n")
 :"No proc or DoT spell activity";

export function DamageFighterBars({fighters}:{fighters:DamageFighterBarRow[]}){
 const maxDamage=Math.max(1,...fighters.map(fighter=>fighter.totalDamage));
 return <div className="meter-players">{fighters.map((fighter,index)=>{
  const delta=fighter.contributionDelta||0,effect=fighter.effects;
  const damageFill=fighter.totalDamage/maxDamage*100;
  const damageLeader=fighter.totalDamage>0&&fighter.totalDamage===maxDamage;
  return <div key={fighter.name} className={`meter-player${fighter.mine?" is-me":""}${damageLeader?" is-damage-leader":""}`} style={{"--meter-color":damageBarColors[index%damageBarColors.length]} as CSSProperties}>
   <div className="meter-contribution" style={{width:damageFill+"%"}} title={`${damageFill.toFixed(1)}% of the leading total damage`} aria-hidden="true"/>
   <span className="meter-rank">#{fighter.rank}</span>
   <div className="meter-name"><div className="meter-identity"><strong title={fighter.name}>{fighter.name}</strong><time title={fighter.combatTimeTitle||"Time since this fighter's first outgoing hit"}>{formatFighterDuration(fighter.combatSeconds)}</time>{fighter.mine&&<em>ME</em>}</div><div className="meter-incoming" title={`Incoming damage recorded against ${fighter.name}`}><strong>{number(fighter.incomingDamage)}</strong><span>INCOMING DMG</span></div></div>
   <div className="meter-share"><span>{delta>.05?"+":delta<-.05?"-":""} {fighter.contribution.toFixed(1)}%</span><small>{fighter.shareDetail||"share"}</small></div>
   <div className="meter-effects" title={fighter.effectsTitle} aria-label={`${effect.procCount} procs, ${effect.procDirectDamage} direct proc damage, ${effect.procDotDamage} proc DoT damage; ${effect.spellCount} spells, ${effect.spellDirectDamage} direct spell damage, ${effect.spellDotDamage} spell DoT damage`}>
    <span><b>PROCS {effect.procCount}</b><i>/ DD {number(effect.procDirectDamage)}</i><em>/ DOT {number(effect.procDotDamage)}</em></span>
    {(effect.spellCount>0||effect.spellDirectDamage>0||effect.spellDotDamage>0)&&<span><b>SPELL {effect.spellCount}</b><i>/ DD {number(effect.spellDirectDamage)}</i><em>/ DOT {number(effect.spellDotDamage)}</em></span>}
   </div>
   <div className="meter-stat"><span>{fighter.dps.toFixed(1)}<small>DPS</small></span><strong>{number(fighter.totalDamage)}<small>DMG</small></strong></div>
  </div>;
 })}</div>;
}
