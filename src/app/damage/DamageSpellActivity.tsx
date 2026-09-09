import type {CSSProperties} from "react";
import type {DamageEncounter,DamageSpellMetric,TrackedSpellActivity} from "../../shared/contracts";

const colors=["#fb56a3","#a878fa","#57c7ff","#ffa357","#7bdfb2","#f37d8d"];
const number=(value:number)=>Math.round(value).toLocaleString();
const stamp=(value:string)=>Date.parse(value.includes("T")?value:value.replace(" ","T"));

export interface SpellPlayerRanking {
 name:string;
 rank:number;
 knownDamage:number;
 landings:number;
 direct:number;
 procs:number;
 itemClicks:number;
 unknown:number;
 spells:number;
}

export function rankSpellPlayers(metrics:DamageSpellMetric[],activity:TrackedSpellActivity[]):SpellPlayerRanking[]{
 const players=new Map<string,Omit<SpellPlayerRanking,"rank">>();
 const get=(name:string)=>{
  const key=name.trim().toLowerCase()||"unknown";
  let row=players.get(key);
  if(!row){row={name:name.trim()||"Unknown",knownDamage:0,landings:0,direct:0,procs:0,itemClicks:0,unknown:0,spells:0};players.set(key,row)}
  return row;
 };
 for(const metric of metrics){
  const row=get(metric.playerName);
  row.knownDamage+=metric.directProcDamage+metric.dotDamage;
 }
 const spellSets=new Map<string,Set<string>>();
 for(const cast of activity){
  const key=cast.casterName.trim().toLowerCase()||"unknown",row=get(cast.casterName);
  row.landings+=1;
  if(cast.sourceKind==="direct")row.direct+=1;
  else if(cast.sourceKind==="proc")row.procs+=1;
  else if(cast.sourceKind==="item_click")row.itemClicks+=1;
  else row.unknown+=1;
  const spells=spellSets.get(key)||new Set<string>();
  spells.add(cast.spellName.toLowerCase());
  spellSets.set(key,spells);
 }
 for(const [key,row] of players)row.spells=spellSets.get(key)?.size||0;
 return [...players.values()]
  .sort((a,b)=>b.knownDamage-a.knownDamage||b.landings-a.landings||a.name.localeCompare(b.name))
  .map((row,index)=>({...row,rank:index+1}));
}

export function spellSourceLabel(activity:TrackedSpellActivity):string{
 if(activity.sourceKind==="direct")return "Direct cast";
 if(activity.sourceKind==="proc")return "Proc";
 if(activity.sourceKind==="item_click")return activity.sourceName?`Item click - ${activity.sourceName}`:"Item click";
 return activity.sourceName?`Unattributed spell - possible ${activity.sourceName} item click`:"Unattributed spell";
}

export function DamageSpellRankingBars({metrics,activity}:{metrics:DamageSpellMetric[];activity:TrackedSpellActivity[]}){
 const rows=rankSpellPlayers(metrics,activity),max=Math.max(1,...rows.map(row=>row.knownDamage));
 if(!rows.length)return <div className="tracked-spells-empty">No recognized spell landings yet.</div>;
 return <div className="spell-ranking-bars" aria-label="Player spell rankings">{rows.map((row,index)=><article key={row.name} style={{"--spell-rank-color":colors[index%colors.length]} as CSSProperties}>
  <i style={{width:(row.knownDamage/max*100)+"%"}}/>
  <b>#{row.rank}</b><strong title={row.name}>{row.name}</strong>
  <span><em>{number(row.knownDamage)}</em><small>KNOWN DMG</small></span>
  <span><em>{row.landings}</em><small>LANDINGS</small></span>
  <div><small>DIRECT {row.direct}</small><small>PROC {row.procs}</small><small>ITEM {row.itemClicks}</small>{row.unknown>0&&<small>UNKNOWN {row.unknown}</small>}</div>
 </article>)}</div>;
}

export function TrackedSpellsPanel({row}:{row:DamageEncounter}){
 const activity=[...(row.trackedSpells||[])].sort((a,b)=>stamp(b.happenedAt)-stamp(a.happenedAt)||b.id-a.id);
 const metrics=row.spellMetrics||[];
 if(!activity.length&&!metrics.length)return null;
 return <section className="tracked-spells"><header><div><span className="eyebrow">Recognized spell activity</span><h3>Tracked spells</h3></div><strong>{activity.length} recent landing{activity.length===1?"":"s"}</strong></header>
  <DamageSpellRankingBars metrics={metrics} activity={activity}/>
  {activity.length>0&&<div className="tracked-spell-feed">{activity.map(cast=><article key={cast.id}>
   <div><strong>{cast.spellName}</strong><small>{cast.casterName} on {cast.targetName}</small></div>
   <em className={`spell-source ${cast.sourceKind}`}>{spellSourceLabel(cast)}</em>
   <time>{new Date(stamp(cast.happenedAt)).toLocaleTimeString([],{hour:"numeric",minute:"2-digit",second:"2-digit"})}</time>
  </article>)}</div>}
 </section>;
}