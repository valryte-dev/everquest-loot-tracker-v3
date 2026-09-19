import {useMemo,useState} from "react";
import {openUrl} from "@tauri-apps/plugin-opener";
import type {AppSnapshot,QuestCatalogItem} from "../../shared/contracts";
import {money} from "../ui";
import {buildQuestCards,buildVeliousArmorTypeCards,groupQuestCards,questCardFaction,VELIOUS_FACTIONS,type QuestCardView} from "./model";
import "./quest-items.css";

const CATEGORIES=[
 {key:"plane_of_sky" as const,label:"Plane of Sky",short:"Sky",description:"Class test rewards and required island drops."},
 {key:"velious_armor" as const,label:"Velious Armor",short:"Armor",description:"Armor turn-in pieces only; gems remain on Armor Gems."},
 {key:"epic" as const,label:"Epic Quests",short:"Epic",description:"Validated inventory items from each class epic checklist."},
];
const icon=(id?:number)=>id?`/quest-items/Item_${id}.png`:"";

export function QuestItemsPage({data}:{data:AppSnapshot}){
 const[category,setCategory]=useState<QuestCatalogItem["category"]>("plane_of_sky");
 const[className,setClassName]=useState("All classes");
 const[faction,setFaction]=useState("All factions");
 const[veliousView,setVeliousView]=useState<"armor"|"class">("armor");
 const[filter,setFilter]=useState("");
 const[ownedOnly,setOwnedOnly]=useState(false);
 const[readyOnly,setReadyOnly]=useState(false);
 const[sort,setSort]=useState<"readiness"|"class"|"name">("readiness");
 const[expanded,setExpanded]=useState<Set<string>>(new Set());
 const cards=useMemo(()=>buildQuestCards(data.questCatalog,data.inventory),[data.questCatalog,data.inventory]);
 const classes=useMemo(()=>[...new Set(cards.filter(card=>card.category===category).map(card=>card.className))].sort(),[cards,category]);
 const displayCards=useMemo(()=>category==="velious_armor"&&veliousView==="armor"?buildVeliousArmorTypeCards(cards):cards,[cards,category,veliousView]);
 const visible=useMemo(()=>{
  const query=filter.trim().toLocaleLowerCase();
  return displayCards.filter(card=>card.category===category)
   .filter(card=>category==="velious_armor"&&veliousView==="armor"||className==="All classes"||card.className===className)
   .filter(card=>category!=="velious_armor"||faction==="All factions"||questCardFaction(card)===faction)
   .filter(card=>!ownedOnly||card.owned>0)
   .filter(card=>!readyOnly||card.ready)
   .filter(card=>!query||[
    card.className,card.questName,card.rewardName,
    ...card.components.flatMap(component=>[component.itemName,component.slot,component.faction,...component.holders.map(holder=>holder.character)])
   ].join(" ").toLocaleLowerCase().includes(query))
   .sort((a,b)=>sort==="class"?a.className.localeCompare(b.className)||a.questName.localeCompare(b.questName)
    :sort==="name"?a.rewardName.localeCompare(b.rewardName)
    :(b.owned/Math.max(1,b.required))-(a.owned/Math.max(1,a.required))||a.className.localeCompare(b.className));
 },[displayCards,category,className,faction,filter,ownedOnly,readyOnly,sort,veliousView]);
 const grouped=useMemo(()=>groupQuestCards(visible,category),[visible,category]);
 const categoryCards=displayCards.filter(card=>card.category===category);
 const componentTypes=new Map(categoryCards.flatMap(card=>card.components).map(component=>[
  component.itemId!=null?`id:${component.itemId}`:`name:${component.itemName.toLocaleLowerCase()}`,
  component
 ]));
 const uniqueComponents=[...componentTypes.values()];
 const distinctHeld=uniqueComponents.filter(component=>component.held>0).length;
 const totalTypes=uniqueComponents.length;
 const heldCount=uniqueComponents.reduce((sum,component)=>sum+component.held,0);
 const heldValue=uniqueComponents.reduce((sum,component)=>sum+component.held*(component.valuePp||0),0);
 const ready=categoryCards.filter(card=>card.ready).length;
 const changeCategory=(next:QuestCatalogItem["category"])=>{setCategory(next);setClassName("All classes");setFaction("All factions");setExpanded(new Set())};
 const allExpanded=visible.length>0&&visible.every(card=>expanded.has(card.key));
 return <>
  <section className="quest-stats">
   <article><span>Components found</span><strong>{distinctHeld} / {totalTypes}</strong></article>
   <article><span>Total pieces held</span><strong>{heldCount}</strong></article>
   <article><span>Quests ready</span><strong>{ready}</strong></article>
   <article><span>Estimated held value</span><strong>{money(heldValue)}</strong></article>
  </section>
  <section className="quest-category-tabs">{CATEGORIES.map(item=><button key={item.key} className={category===item.key?"active":""} onClick={()=>changeCategory(item.key)}><strong>{item.label}</strong><small>{item.description}</small></button>)}</section>
  <section className="quest-toolbar">
   <label className="search"><input value={filter} onChange={event=>setFilter(event.target.value)} placeholder="Filter quests, items, slots, or characters..."/><button disabled={!filter} onClick={()=>setFilter("")} aria-label="Clear quest filter">x</button></label>
   {!(category==="velious_armor"&&veliousView==="armor")&&<select aria-label="Filter by class" value={className} onChange={event=>setClassName(event.target.value)}><option>All classes</option>{classes.map(name=><option key={name}>{name}</option>)}</select>}
   {category==="velious_armor"&&<select aria-label="Filter by Velious faction" value={faction} onChange={event=>setFaction(event.target.value)}><option>All factions</option>{VELIOUS_FACTIONS.map(name=><option key={name}>{name}</option>)}</select>}
   {category==="velious_armor"&&<div className="quest-view-switch" role="group" aria-label="Velious armor view"><button className={veliousView==="armor"?"active":""} onClick={()=>{setVeliousView("armor");setClassName("All classes")}}>Armor type</button><button className={veliousView==="class"?"active":""} onClick={()=>setVeliousView("class")}>Class</button></div>}
   <select aria-label="Sort quests" value={sort} onChange={event=>setSort(event.target.value as typeof sort)}><option value="readiness">Sort: readiness</option><option value="class">Sort: class</option><option value="name">Sort: reward</option></select>
   <label className="check-line"><input type="checkbox" checked={ownedOnly} onChange={event=>setOwnedOnly(event.target.checked)}/> Has pieces</label>
   <label className="check-line"><input type="checkbox" checked={readyOnly} onChange={event=>setReadyOnly(event.target.checked)}/> Ready only</label>
   <button onClick={()=>setExpanded(allExpanded?new Set():new Set(visible.map(card=>card.key)))}>{allExpanded?"Collapse all":"Expand all"}</button>
  </section>
  <div className="quest-result-count">{visible.length} quest panels - {CATEGORIES.find(item=>item.key===category)?.short} catalog</div>
  <section className="quest-groups">{grouped.map(group=><section className="quest-group" key={group.key}>{group.label&&<header><div><span>{category==="velious_armor"?"Quest hub":"Class"}</span><h2>{group.label}</h2></div><small>{group.cards.length} quest panel{group.cards.length===1?"":"s"} - {group.cards.filter(card=>card.ready).length} ready</small></header>}<div className="quest-card-grid">{group.cards.map(card=><QuestCard key={card.key} card={card} expanded={expanded.has(card.key)} toggle={()=>setExpanded(current=>{const next=new Set(current);next.has(card.key)?next.delete(card.key):next.add(card.key);return next})}/>)}</div></section>)}</section>
  {!visible.length&&<section className="quest-empty"><strong>No quest panels match these filters.</strong><button onClick={()=>{setFilter("");setClassName("All classes");setFaction("All factions");setOwnedOnly(false);setReadyOnly(false)}}>Reset filters</button></section>}
 </>;
}

function QuestCard({card,expanded,toggle}:{card:QuestCardView;expanded:boolean;toggle:()=>void}){
 const progress=Math.round(card.owned/Math.max(1,card.required)*100);
 const preview=[...card.components].sort((a,b)=>b.held-a.held||a.itemName.localeCompare(b.itemName)).slice(0,6);
 const epicPreview=card.category==="epic";
 return <article className={`quest-card ${card.ready?"ready":card.owned?"started":"missing"} ${expanded?"expanded":""}`}>
  <header>
   <button className="quest-card-toggle" onClick={toggle} aria-expanded={expanded}>
    <span className="quest-reward-icon">{card.rewardIconId?<img src={icon(card.rewardIconId)} alt=""/>:<b>{card.className.slice(0,2)}</b>}</span>
    <span><small>{card.className}</small><strong>{card.rewardName}</strong><em>{card.owned} of {card.required} requirements available</em></span>
   </button>
   <button className="quest-source" title="Open P99 Wiki source" aria-label="Open P99 Wiki source" onClick={()=>openUrl(card.sourceUrl)}>Open</button>
  </header>
  <div className="quest-progress"><i style={{width:`${progress}%`}}/><span>{progress}%</span></div>
  {!expanded&&<div className={`quest-preview ${epicPreview?"quest-preview-vertical":""}`}>{preview.map(component=><span key={component.componentId} className={component.held?"owned":"missing"} title={`${component.itemName}: ${component.held} held`}>{component.iconId?<img src={icon(component.iconId)} alt=""/>:<b>?</b>}{epicPreview&&<em>{component.itemName}</em>}<i>{component.held||"-"}</i></span>)}{epicPreview&&card.components.length>preview.length&&<small className="quest-preview-more">+ {card.components.length-preview.length} more requirements</small>}</div>}
  {expanded&&<div className="quest-components">{card.components.map(component=><article key={component.componentId} className={component.held>=component.quantity?"owned":"missing"}>
   {component.iconId?<img src={icon(component.iconId)} alt=""/>:<span className="quest-icon-fallback">?</span>}
   <div><small>{component.slot||component.faction||"Quest item"}</small><strong>{component.itemName}</strong><em>{component.valuePp?money(component.valuePp):"No stored value"}</em></div>
   <b>{component.held} / {component.quantity}</b>
   {component.holders.length>0&&<details><summary>{component.holders.length} holder{component.holders.length===1?"":"s"}</summary>{component.holders.map(holder=><p key={holder.character}><strong>{holder.character}</strong><span>x{holder.count}</span><small>{holder.locations.join(", ")}</small></p>)}</details>}
  </article>)}</div>}
 </article>;
}
