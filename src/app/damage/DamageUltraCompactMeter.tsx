import type {CSSProperties} from "react";
import type {DamageEncounter} from "../../shared/contracts";
import {damageBarColors,formatFighterDuration,type DamageFighterBarRow} from "./DamageFighterBars";

const number=(value:number)=>Math.round(value).toLocaleString();

export function DamageUltraCompactMeter({row,durationSeconds,fighters}:{row:DamageEncounter;durationSeconds:number;fighters:DamageFighterBarRow[]}){
 const duration=Math.max(1,durationSeconds);
 const maxDamage=Math.max(1,...fighters.map(fighter=>fighter.totalDamage));
 const incomingDps=(row.incomingDamage||0)/duration;
 return <article className="meter-fight meter-ultra">
  <header className="meter-ultra-header">
   <div className="meter-ultra-fight"><span className="meter-pulse"><i/>{row.outcome==="active"?"Live":"Recent"}</span><h2 title={row.mobName}>{row.mobName}</h2><small title={row.weapons.length?`Last known weapon snapshot: ${row.weapons.join(" / ")}`:"No weapon snapshot"}>{row.character} · {formatFighterDuration(duration)} · {fighters.length} fighter{fighters.length===1?"":"s"}</small></div>
   <div className="meter-ultra-group"><span><strong>{(row.totalDamage/duration).toFixed(1)}</strong><small>DPS</small></span><span><strong>{number(row.totalDamage)}</strong><small>DMG</small></span></div>
  </header>
  <section className="meter-ultra-players" aria-label="Damage rankings">{fighters.map((fighter,index)=>{
   const effects=fighter.effects,procDamage=effects.procDirectDamage+effects.procDotDamage;
   return <article key={fighter.name} className={fighter.mine?"is-me":""} style={{"--meter-color":damageBarColors[index%damageBarColors.length]} as CSSProperties}>
    <i style={{"--meter-fill":fighter.totalDamage/maxDamage} as CSSProperties} aria-hidden="true"/>
    <b>#{fighter.rank}</b>
    <div className="meter-ultra-name"><strong title={fighter.name}>{fighter.name}</strong>{fighter.mine&&<em>ME</em>}<small>{formatFighterDuration(fighter.combatSeconds)} · {fighter.contribution.toFixed(1)}% · IN {number(fighter.incomingDamage)}{effects.procCount>0?` · ${effects.procCount}P/${number(procDamage)}`:""}</small></div>
    <div className="meter-ultra-stat"><strong>{fighter.dps.toFixed(1)}</strong><small>DPS</small></div>
    <div className="meter-ultra-stat damage"><strong>{number(fighter.totalDamage)}</strong><small>DMG</small></div>
   </article>;
  })}</section>
  <footer className="meter-ultra-footer"><span><b>IN</b> {number(row.incomingDamage||0)} / {incomingDps.toFixed(1)} DPS</span><span><b>PROC</b> {row.procCount||0} / {number(row.totalProcDamage??((row.directProcDamage||0)+(row.procDotDamage||0)))}</span><span><b>DOT</b> {number(row.dotDamage||0)}</span><span><b>HITS</b> {number(row.hitCount)} · MAX {number(row.maxHit)}</span></footer>
 </article>;
}