import {useMemo,useState} from "react";
import {openUrl} from "@tauri-apps/plugin-opener";
import type {AppSnapshot} from "../../shared/contracts";
import {money} from "../ui";
import {ARMOR_ARCHETYPES,ARMOR_SLOTS,VELIOUS_ARMOR_GEMS,VELIOUS_GEM_SOURCE,gemFor,type ArmorArchetype} from "./catalog";

interface GemHolding { character:string; count:number; locations:Set<string> }

export function VeliousGemsPage({data}:{data:AppSnapshot}){
 const[filter,setFilter]=useState("");
 const inventoryByGem=useMemo(()=>{
  const catalog=new Set(VELIOUS_ARMOR_GEMS.map(gem=>gem.name.toLocaleLowerCase()));
  const result=new Map<string,Map<string,GemHolding>>();
  for(const item of data.inventory){
   const key=item.itemName.toLocaleLowerCase();
   if(!catalog.has(key))continue;
   if(!result.has(key))result.set(key,new Map());
   const holders=result.get(key)!;
   const holder=holders.get(item.character)||{character:item.character,count:0,locations:new Set<string>()};
   holder.count+=item.count;holder.locations.add(item.location);holders.set(item.character,holder);
  }
  return result;
 },[data.inventory]);
 const total=[...inventoryByGem.values()].flatMap(holders=>[...holders.values()]).reduce((sum,holder)=>sum+holder.count,0);
 const characters=new Set([...inventoryByGem.values()].flatMap(holders=>[...holders.keys()]));
 const estimated=VELIOUS_ARMOR_GEMS.reduce((sum,gem)=>{const count=[...(inventoryByGem.get(gem.name.toLocaleLowerCase())?.values()||[])].reduce((value,holder)=>value+holder.count,0);const market=data.items.find(item=>item.name.toLocaleLowerCase()===gem.name.toLocaleLowerCase())?.valuePp||0;return sum+count*market},0);
 return <>
  <section className="gem-stats"><article><span>Distinct gems found</span><strong>{inventoryByGem.size} / {VELIOUS_ARMOR_GEMS.length}</strong></article><article><span>Total gems held</span><strong>{total}</strong></article><article><span>Characters holding gems</span><strong>{characters.size}</strong></article><article><span>Estimated 30-day value</span><strong>{money(estimated)}</strong></article></section>
  <section className="gem-toolbar"><div className="search"><input value={filter} onChange={event=>setFilter(event.target.value)} placeholder="Filter gems, slots, or characters…"/><button disabled={!filter} onClick={()=>setFilter("")} aria-label="Clear gem filter">×</button></div><button onClick={()=>openUrl(VELIOUS_GEM_SOURCE)}>P99 Wiki source ↗</button></section>
  <div className="gem-panels">{ARMOR_ARCHETYPES.map(archetype=><GemPanel key={archetype} archetype={archetype} filter={filter} inventoryByGem={inventoryByGem} items={data.items}/>)}</div>
 </>;
}

function GemPanel({archetype,filter,inventoryByGem,items}:{archetype:ArmorArchetype;filter:string;inventoryByGem:Map<string,Map<string,GemHolding>>;items:AppSnapshot["items"]}){
 const rows=ARMOR_SLOTS.map(slot=>({slot,gem:gemFor(archetype,slot)})).filter(row=>row.gem).filter(({slot,gem})=>{if(!filter)return true;const holders=[...(inventoryByGem.get(gem!.name.toLocaleLowerCase())?.keys()||[])];return `${archetype} ${slot} ${gem!.name} ${holders.join(" ")}`.toLocaleLowerCase().includes(filter.toLocaleLowerCase())});
 const found=ARMOR_SLOTS.filter(slot=>{const gem=gemFor(archetype,slot);return gem&&inventoryByGem.has(gem.name.toLocaleLowerCase())}).length;
 return <section className="gem-panel"><header><div className={`gem-archetype ${archetype.toLocaleLowerCase()}`}>{archetype[0]}</div><div><h2>{archetype} armor gems</h2><span>{found} of {ARMOR_SLOTS.length} slots represented</span></div></header><div className="gem-slot-grid">{rows.map(({slot,gem})=>{const holders=[...(inventoryByGem.get(gem!.name.toLocaleLowerCase())?.values()||[])].sort((a,b)=>a.character.localeCompare(b.character));const count=holders.reduce((sum,holder)=>sum+holder.count,0);const market=items.find(item=>item.name.toLocaleLowerCase()===gem!.name.toLocaleLowerCase())?.valuePp||0;return <article key={slot} className={count?"owned":"missing"}><img src={gem!.icon} alt=""/><div className="gem-copy"><span>{slot}</span><strong>{gem!.name}</strong><small>{market?`${money(market)} each`:"No stored market value"}</small></div>{count>0&&<b>{count}</b>}{count?<details><summary>{holders.length} character{holders.length===1?"":"s"}</summary><div>{holders.map(holder=><p key={holder.character}><strong>{holder.character}</strong><span>×{holder.count}</span><small>{[...holder.locations].join(", ")}</small></p>)}</div></details>:<em>Missing across roster</em>}</article>})}{!rows.length&&<div className="gem-no-match">No {archetype.toLocaleLowerCase()} gems match this filter.</div>}</div></section>}
