import type {CSSProperties} from "react";
import type {IncomingTarget} from "./meterModel";
import {damageBarColors} from "./DamageFighterBars";

const number=(value:number)=>Math.round(value).toLocaleString();
export const totalIncomingDps=(totalDamage:number,durationSeconds:number)=>totalDamage/Math.max(1,durationSeconds);

export function IncomingDamageTotal({totalDamage,durationSeconds}:{totalDamage:number;durationSeconds:number}){
 return <div className="meter-incoming-total"><b>{number(totalDamage)}</b><small>{totalIncomingDps(totalDamage,durationSeconds).toFixed(1)} DPS</small></div>;
}

export function DamageIncomingBars({targets,activeCharacter}:{targets:IncomingTarget[];activeCharacter:string}){
 const maxDamage=Math.max(1,...targets.map(target=>target.totalDamage));
 if(!targets.length)return <div className="meter-incoming-empty">No incoming damage yet.</div>;
 return <div className="meter-incoming-bars">{targets.map((target,index)=>{
  const fill=target.totalDamage/maxDamage*100;
  const mine=target.name.toLowerCase()===activeCharacter.toLowerCase();
  return <article key={target.name} className={`${mine?" is-me":""}${target.totalDamage===maxDamage?" is-damage-leader":""}`} style={{"--meter-color":damageBarColors[index%damageBarColors.length]} as CSSProperties}>
   <i style={{width:fill+"%"}} title={`${fill.toFixed(1)}% of the leading incoming damage`} aria-hidden="true"/>
   <b>#{target.rank}</b>
   <div><strong title={target.name}>{target.name}</strong>{mine&&<em>ME</em>}<small>{target.hitCount} hit{target.hitCount===1?"":"s"} · max {number(target.maxHit)}</small></div>
   <span><strong>{number(target.totalDamage)}</strong><small>TAKEN</small></span>
   <span><strong>{target.contribution.toFixed(1)}%</strong><small>SHARE</small></span>
  </article>;
 })}</div>;
}