import {useCallback,useEffect,useMemo,useState} from "react";
import {getSpellCatalogEntries} from "../shared/backend";
import type {SpellInfo} from "../shared/contracts";
import {DataTable,type Column} from "./ui";

export function SpellCatalogBrowser(){
 const[rows,setRows]=useState<SpellInfo[]>([]),[loading,setLoading]=useState(true),[error,setError]=useState("");
 const load=useCallback(async()=>{setLoading(true);try{setRows(await getSpellCatalogEntries());setError("")}catch(value){setError(String(value).replace(/^Error:\s*/,""))}finally{setLoading(false)}},[]);
 useEffect(()=>{void load()},[load]);
 const counts=useMemo(()=>rows.reduce((result,row)=>{result[row.damageKind]=(result[row.damageKind]||0)+1;return result},{} as Record<string,number>),[rows]);
 const columns:Column<SpellInfo>[]=[
  {key:"name",label:"Spell",value:row=>row.spellName,render:row=><a href={row.wikiUrl} target="_blank" rel="noreferrer"><strong>{row.spellName}</strong></a>},
  {key:"kind",label:"Damage Class",value:row=>row.damageKind,render:row=><span className={`pill spell-kind-${row.damageKind}`}>{row.damageKind.replace("_"," ")}</span>},
  {key:"classes",label:"Classes",value:row=>row.classes.map(value=>`${value.name} ${value.level}`).join(", ")},
  {key:"effects",label:"Effects",value:row=>row.effects.map(value=>value.description).join("; ")},
  {key:"cast",label:"Cast on Other",value:row=>row.castOnOther||""},
  {key:"tick",label:"Per Tick",value:row=>row.damagePerTick||0,render:row=>row.damagePerTick?.toLocaleString()||"-"},
  {key:"ticks",label:"Ticks",value:row=>row.tickCount||0,render:row=>row.tickCount||"-"},
  {key:"total",label:"DoT Total",value:row=>row.totalDotDamage||0,render:row=>row.totalDotDamage?.toLocaleString()||"-"},
  {key:"duration",label:"Duration",value:row=>row.duration},
 ];
 return <section className="card spell-catalog-browser"><header><div><h2>Game spell catalog</h2><p>Inspect cached wiki fields used by live spell and damage parsing. Every grid can be filtered and sorted.</p></div><button disabled={loading} onClick={()=>void load()}>{loading?"Loading...":"Refresh view"}</button></header>
  <div className="spell-browser-stats"><span><b>{rows.length.toLocaleString()}</b> spells</span><span><b>{(counts.dot||0).toLocaleString()}</b> DoT</span><span><b>{(counts.direct||0).toLocaleString()}</b> direct</span><span><b>{(counts.hybrid||0).toLocaleString()}</b> hybrid</span></div>
  {error?<div className="spell-sync-error">{error}</div>:<DataTable rows={rows} columns={columns} rowKey={row=>row.spellName} empty={loading?"Loading the local spell catalog...":"No spell metadata has been cached yet. Reload it from System."}/>} 
 </section>;
}