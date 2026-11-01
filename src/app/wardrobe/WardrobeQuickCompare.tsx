import {useMemo} from "react";
import type {WardrobeCatalogItem} from "../../shared/contracts";
import {ItemHover} from "../items/ItemHover";
import {money} from "../ui";
import {itemIconPath,PAPERDOLL_SLOT_GROUPS,type EquipmentSlotKey} from "../characters/EquipmentPaperDoll";
import {quickCompareBuild,quickCompareSelectedUpgrades,statLabel,wardrobeStatDeltas,wardrobeTotals,wardrobeValueSummary,weaponRatio,type WardrobeCompareEntry,type WardrobeComparePriority,type WardrobeMarketLookup,type WardrobeStatKey} from "./model";

const slots=PAPERDOLL_SLOT_GROUPS.flat();
const slotLabels=new Map(slots.map(slot=>[slot.key,slot.label]));
const COMBAT_STATS:WardrobeStatKey[]=["ac","hp","mana","attack","haste","manaRegen","damageShield"];
const ATTRIBUTE_STATS:WardrobeStatKey[]=["strength","stamina","agility","dexterity","intelligence","wisdom","charisma"];
const RESIST_STATS:WardrobeStatKey[]=["magicResist","fireResist","coldResist","diseaseResist","poisonResist"];
const PRIORITIES:{value:WardrobeComparePriority;label:string}[]=[{value:1,label:"High"},{value:0,label:"Normal"},{value:2,label:"Low"}];

interface WardrobeQuickCompareProps {
 entries:WardrobeCompareEntry[];
 equipped:Partial<Record<EquipmentSlotKey,WardrobeCatalogItem>>;
 market:WardrobeMarketLookup;
 onAdd:()=>void;
 onChange:(entries:WardrobeCompareEntry[])=>void;
 onApply:(items:Partial<Record<EquipmentSlotKey,WardrobeCatalogItem>>)=>void;
}

const itemValues=(items:Partial<Record<EquipmentSlotKey,WardrobeCatalogItem>>)=>Object.values(items).filter((item):item is WardrobeCatalogItem=>Boolean(item));
const signed=(value:number)=>value>0?`+${value}`:String(value);
const percentage=(delta:number,baseline:number)=>baseline?`${Math.round(Math.abs(delta/baseline)*100)}%`:"";

function Delta({value,baseline=0}:{value:number;baseline?:number}){
 if(!value)return <span className="quick-compare-delta neutral">—</span>;
 const percent=percentage(value,baseline);
 return <span className={`quick-compare-delta ${value>0?"positive":"negative"}`}>{signed(value)}{percent&&<small> ({percent})</small>}</span>;
}

function ItemDelta({entry,baseline,winner}:{entry:WardrobeCompareEntry;baseline?:WardrobeCatalogItem;winner:boolean}){
 const deltas=wardrobeStatDeltas(entry.item,baseline);
 const changed=Object.entries(deltas) as [WardrobeStatKey,number][];
 return <div className="quick-compare-item-delta">
  <span>vs {baseline?.name||"empty slot"}</span>
  {!winner&&entry.enabled&&<em>Another {slotLabels.get(entry.slotKey)} item has higher priority</em>}
  {changed.length?<div>{changed.map(([key,value])=><b key={key} className={value>0?"positive":"negative"}>{statLabel(key)} {signed(value)}</b>)}</div>:<small>No stat change</small>}
 </div>;
}

function WeaponLine({label,item}:{label:string;item?:WardrobeCatalogItem}){
 return <div><span>{label}</span>{item?<strong>{item.name}<small>{item.damage>0?`${item.damage} / ${item.delay} · ratio ${weaponRatio(item).toFixed(2)}`:"Non-weapon item"}</small></strong>:<em>Empty</em>}</div>;
}

function TotalsGroup({title,keys,current,compared}:{title:string;keys:WardrobeStatKey[];current:Record<string,number>;compared:Record<string,number>}){
 return <section className="quick-compare-total-group"><h4>{title}</h4><div>{keys.map(key=>{
  const before=current[key]||0,after=compared[key]||0,delta=after-before;
  const cap=ATTRIBUTE_STATS.includes(key)?255:key==="haste"?100:undefined;
  return <article key={key} className={cap!==undefined&&after>cap?"capped":""}><span>{statLabel(key)}</span><strong>{after}{cap!==undefined&&<small>/{cap}</small>}</strong><Delta value={delta} baseline={before}/></article>;
 })}</div></section>;
}

export function WardrobeQuickCompare({entries,equipped,market,onAdd,onChange,onApply}:WardrobeQuickCompareProps){
 const compared=useMemo(()=>quickCompareBuild(equipped,entries),[equipped,entries]);
 const currentTotals=useMemo(()=>wardrobeTotals(itemValues(equipped)),[equipped]);
 const comparedTotals=useMemo(()=>wardrobeTotals(itemValues(compared)),[compared]);
 const upgradeItems=useMemo(()=>quickCompareSelectedUpgrades(equipped,entries),[equipped,entries]);
 const upgradeValue=useMemo(()=>wardrobeValueSummary(upgradeItems,market),[market,upgradeItems]);
 const projectedValue=useMemo(()=>wardrobeValueSummary(itemValues(compared),market),[compared,market]);
 const currentWeight=itemValues(equipped).reduce((sum,item)=>sum+item.weight,0);
 const comparedWeight=itemValues(compared).reduce((sum,item)=>sum+item.weight,0);
 const enabledCount=entries.filter(entry=>entry.enabled).length;
 const changed=slots.some(slot=>(equipped[slot.key]?.id||0)!==(compared[slot.key]?.id||0));
 const update=(id:string,patch:Partial<WardrobeCompareEntry>)=>onChange(entries.map(entry=>entry.id===id?{...entry,...patch}:entry));
 return <section className="card wardrobe-quick-compare">
  <header><div><span className="eyebrow">Upgrade sandbox</span><h2>Quick Compare</h2><p>Shortlist items and see your combined totals if they were equipped.</p></div><div className="quick-compare-actions"><button type="button" onClick={onAdd}>+ Add item</button><button type="button" disabled={!entries.length} onClick={()=>onChange([])}>Clear list</button><button type="button" className="primary" disabled={!enabledCount||!changed} onClick={()=>onApply(compared)}>Apply to wardrobe</button></div></header>
  <div className="quick-compare-layout">
   <section className="quick-compare-list" aria-label="Items in quick compare">
    <div className="quick-compare-list-heading"><strong>{entries.length} candidate{entries.length===1?"":"s"}</strong><span>{enabledCount} included</span></div>
    {!entries.length?<div className="quick-compare-empty"><strong>No items yet</strong><p>Add an item to compare it against the gear currently on the model.</p><button type="button" onClick={onAdd}>Add your first item</button></div>:<ol>{entries.map(entry=>{
     const baseline=equipped[entry.slotKey];
     const winner=compared[entry.slotKey]?.id===entry.item.id;
     return <li key={entry.id} className={!entry.enabled?"disabled":winner?"winner":"superseded"}>
      <div className="quick-compare-item-main"><input type="checkbox" checked={entry.enabled} onChange={()=>update(entry.id,{enabled:!entry.enabled})} aria-label={`Include ${entry.item.name} in comparison`}/><ItemHover itemName={entry.item.name} itemId={entry.item.peqId||entry.item.id} iconId={entry.item.iconId} valuePp={market(entry.item)?.valuePp} details={entry.item} focusable={false}><span className="quick-compare-icon">{itemIconPath(entry.item.iconId)?<img src={itemIconPath(entry.item.iconId)} alt=""/>:<i>?</i>}</span></ItemHover><div><ItemHover itemName={entry.item.name} itemId={entry.item.peqId||entry.item.id} iconId={entry.item.iconId} valuePp={market(entry.item)?.valuePp} details={entry.item}><strong>{entry.item.name}</strong></ItemHover><small>{slotLabels.get(entry.slotKey)||entry.slotKey} · {market(entry.item)?.valuePp?money(market(entry.item)?.valuePp):"No estimate"}</small></div><select value={entry.priority} onChange={event=>update(entry.id,{priority:Number(event.target.value) as WardrobeComparePriority})} aria-label={`Priority for ${entry.item.name}`}>{PRIORITIES.map(priority=><option key={priority.value} value={priority.value}>{priority.label}</option>)}</select><button type="button" className="quick-compare-remove" onClick={()=>onChange(entries.filter(value=>value.id!==entry.id))} title={`Remove ${entry.item.name}`} aria-label={`Remove ${entry.item.name}`}>×</button></div>
      {entry.enabled&&<ItemDelta entry={entry} baseline={baseline} winner={winner}/>}<input className="quick-compare-note" type="text" maxLength={80} value={entry.note} onChange={event=>update(entry.id,{note:event.target.value})} placeholder="Add a note (drop, price, who it is for…)" aria-label={`Note for ${entry.item.name}`}/>
     </li>;
    })}</ol>}
   </section>
   <aside className="quick-compare-totals"><header><div><span className="eyebrow">If equipped</span><h3>Combined totals</h3></div><span>Δ vs equipped</span></header>
    <section className="quick-compare-value-summary" aria-label="Comparison market values"><article><span>Compared upgrades</span><strong>{money(upgradeValue.totalPp)}</strong><small>{upgradeValue.pricedCount} priced{upgradeValue.missingCount?` · ${upgradeValue.missingCount} missing`:""}</small></article><article><span>Projected outfit value</span><strong>{money(projectedValue.totalPp)}</strong><small>{projectedValue.pricedCount} of {itemValues(compared).length} equipped items priced</small></article></section>
    <TotalsGroup title="Combat" keys={COMBAT_STATS} current={currentTotals} compared={comparedTotals}/><TotalsGroup title="Attributes" keys={ATTRIBUTE_STATS} current={currentTotals} compared={comparedTotals}/><TotalsGroup title="Resists" keys={RESIST_STATS} current={currentTotals} compared={comparedTotals}/>
    <section className="quick-compare-weapons"><h4>Weapons</h4><WeaponLine label="Primary" item={compared.primary}/><WeaponLine label="Secondary" item={compared.secondary}/></section>
    <div className="quick-compare-weight"><span>Total weight</span><strong>{comparedWeight.toFixed(1)}</strong><Delta value={Number((comparedWeight-currentWeight).toFixed(1))} baseline={currentWeight}/></div>
   </aside>
  </div>
 </section>;
}