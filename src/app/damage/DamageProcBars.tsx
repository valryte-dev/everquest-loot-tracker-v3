import type {CSSProperties} from "react";
import type {DamageSpellMetric} from "../../shared/contracts";
import {rankProccers} from "./meterModel";

const colors=["#f5bd4f","#fb56a3","#a878fa","#57c7ff","#7bdfb2","#f37d8d"];
const number=(value:number)=>Math.round(value).toLocaleString();

export function DamageProcBars({metrics}:{metrics:DamageSpellMetric[]}){
 const players=rankProccers(metrics).slice(0,6);
 if(!players.length)return null;
 const maxDamage=Math.max(1,...players.map(player=>player.totalDamage));
 return <section className="damage-proc-rankings">
  <header><div><span>Proc leaders</span><strong>Top proccers</strong></div><small>Calculated proc damage</small></header>
  <div>{players.map((player,index)=><article key={player.name} style={{"--proc-color":colors[index%colors.length]} as CSSProperties}>
   <i style={{width:`${player.totalDamage/maxDamage*100}%`}}/>
   <b>#{player.rank}</b>
   <strong title={player.name}>{player.name}</strong>
   <span>{player.procCount} proc{player.procCount===1?"":"s"}</span>
   <small>DD {number(player.directDamage)} / DoT {number(player.dotDamage)}</small>
   <em>{number(player.totalDamage)} DMG</em>
  </article>)}</div>
 </section>;
}