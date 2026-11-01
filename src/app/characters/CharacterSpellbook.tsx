import {useCallback,useEffect,useMemo,useRef,useState} from "react";
import {openUrl} from "@tauri-apps/plugin-opener";
import {getSpellCatalogEntries,getSpellCatalogStatus} from "../../shared/backend";
import type {InventoryItem,Spell,SpellInfo} from "../../shared/contracts";
import {SpellHover} from "../SpellHover";
import {DataTable,IconButton,type Column} from "../ui";
import {buildCharacterSpellRows,spellIconPath,type CharacterSpellRow,type CharacterSpellStatus} from "./spellCollectionModel";

type StatusFilter="all"|CharacterSpellStatus;
type LevelFilter="current"|"all";
let catalogRequest:Promise<SpellInfo[]>|undefined;
const loadCatalog=()=>catalogRequest??=(getSpellCatalogEntries().catch(error=>{catalogRequest=undefined;throw error}));

export function CharacterSpellbook({character,className,level,knownSpells,inventory}:{character:string;className:string;level?:number;knownSpells:Spell[];inventory:InventoryItem[]}){
 const[catalog,setCatalog]=useState<SpellInfo[]>([]),[loading,setLoading]=useState(true),[error,setError]=useState("");
 const iconRefreshes=useRef(new Set<string>());
 const[statusFilter,setStatusFilter]=useState<StatusFilter>("all"),[levelFilter,setLevelFilter]=useState<LevelFilter>(level?"current":"all");
 const load=useCallback(async(force=false)=>{
  setLoading(true);setError("");if(force)catalogRequest=undefined;
  try{setCatalog(await loadCatalog())}catch(value){setError(String(value).replace(/^Error:\s*/,""))}finally{setLoading(false)}
 },[]);
 useEffect(()=>{void load()},[load]);
 const missingIconCount=useMemo(()=>catalog.reduce((count,spell)=>count+(spellIconPath(spell.iconLetter)?0:1),0),[catalog]);
 useEffect(()=>{
  if(loading)return;
  let cancelled=false,timer:number|undefined,sawRefresh=false;
  const watch=async()=>{
   try{
    const status=await getSpellCatalogStatus();
    if(cancelled)return;
    if(status.refreshing){sawRefresh=true;timer=window.setTimeout(()=>void watch(),1500);return}
    const version=status.lastRefreshAt||"unversioned";
    if((sawRefresh||missingIconCount>=2)&&!iconRefreshes.current.has(version)){
     iconRefreshes.current.add(version);
     await load(true);
    }
   }catch{/* The visible catalog load error remains the actionable error state. */}
  };
  void watch();
  return()=>{cancelled=true;if(timer!==undefined)window.clearTimeout(timer)};
 },[loading,missingIconCount,load]);
 useEffect(()=>{setStatusFilter("all");setLevelFilter(level?"current":"all")},[character,level]);
 const allRows=useMemo(()=>buildCharacterSpellRows(catalog,className,knownSpells,inventory),[catalog,className,knownSpells,inventory]);
 const levelRows=useMemo(()=>levelFilter==="all"||!level?allRows:allRows.filter(row=>row.level<=level),[allRows,levelFilter,level]);
 const rows=useMemo(()=>statusFilter==="all"?levelRows:levelRows.filter(row=>row.status===statusFilter),[levelRows,statusFilter]);
 const counts=useMemo(()=>levelRows.reduce((result,row)=>{result[row.status]++;return result},{scribed:0,scroll:0,missing:0} as Record<CharacterSpellStatus,number>),[levelRows]);
 const columns:Column<CharacterSpellRow>[]=[
  {key:"status",label:"Status",value:row=>row.status,render:row=><span className={`spell-check-status ${row.status}`}>{row.status==="scribed"?"Scribed":row.status==="scroll"?`Scroll held x${row.scrollCount}`:"Missing"}</span>},
  {key:"level",label:"Lvl",value:row=>row.level},
  {key:"spell",label:"Spell",value:row=>row.info.spellName,render:row=>{const icon=spellIconPath(row.info.iconLetter);return <span className="spell-check-name">{icon?<img src={icon} alt="" loading="lazy"/>:<i aria-hidden="true">?</i>}<strong><SpellHover value={row.info.spellName} force/></strong></span>}},
  {key:"type",label:"Type",value:row=>row.info.spellType||row.info.damageKind,render:row=>row.info.spellType||row.info.damageKind.replace("_"," ")},
  {key:"target",label:"Target",value:row=>row.info.targetType||"-"},
  {key:"mana",label:"Mana",value:row=>numeric(row.info.mana),render:row=>row.info.mana||"-"},
  {key:"skill",label:"School",value:row=>row.info.skill||"-"},
  {key:"cast",label:"Cast",value:row=>numeric(row.info.castingTime),render:row=>seconds(row.info.castingTime)},
  {key:"duration",label:"Duration",value:row=>row.info.duration||"-"},
  {key:"resist",label:"Resist",value:row=>row.info.resist||"-"},
  {key:"find",label:"Find",value:row=>row.info.wikiUrl,render:row=>row.info.wikiUrl?<IconButton icon="external" label={`Find ${row.info.spellName}`} onClick={()=>void openUrl(row.info.wikiUrl)}/>:<span>-</span>},
  {key:"description",label:"Description",value:row=>description(row),render:row=><span className="spell-check-description" title={description(row)}>{description(row)}</span>},
 ];
 if(!className)return <section className="card character-spellbook"><header><div><h2>Spell collection</h2><p>Choose this character's class in the Inventory tab to compare the spellbook with the class catalog.</p></div></header></section>;
 return <section className="card character-spellbook">
  <header><div><h2>{className} spell collection</h2><p>Compare {character}'s imported spellbook and held scrolls with the complete class catalog.</p></div><button disabled={loading} onClick={()=>void load(true)}>{loading?"Loading...":"Refresh catalog"}</button></header>
  <div className="spell-check-summary" aria-label="Spell collection summary">
   <article><span>Catalog</span><strong>{levelRows.length}</strong><small>{levelFilter==="current"&&level?`Through level ${level}`:"All class levels"}</small></article>
   <article className="scribed"><span>Scribed</span><strong>{counts.scribed}</strong><small>{percent(counts.scribed,levelRows.length)} complete</small></article>
   <article className="scroll"><span>Scrolls ready</span><strong>{counts.scroll}</strong><small>Held but not scribed</small></article>
   <article className="missing"><span>Still missing</span><strong>{counts.missing}</strong><small>No scribed spell or scroll</small></article>
  </div>
  <div className="spell-check-toolbar">
   <div className="scope-buttons" aria-label="Filter spells by collection status">{(["all","missing","scroll","scribed"] as StatusFilter[]).map(value=><button key={value} className={statusFilter===value?"active":""} onClick={()=>setStatusFilter(value)}>{value==="all"?"All":value==="scroll"?"Scroll held":value[0].toUpperCase()+value.slice(1)} <b>{value==="all"?levelRows.length:counts[value]}</b></button>)}</div>
   <label><span>Levels</span><select value={levelFilter} onChange={event=>setLevelFilter(event.target.value as LevelFilter)} disabled={!level}><option value="current">Usable now{level?` (1-${level})`:""}</option><option value="all">All class levels</option></select></label>
  </div>
  {error?<div className="spell-check-error"><strong>Could not load the local spell catalog.</strong><span>{error}</span><button onClick={()=>void load(true)}>Try again</button></div>:<DataTable rows={rows} columns={columns} rowKey={row=>`${row.info.spellName}-${row.level}`} rowClass={row=>`spell-check-row ${row.status}`} empty={loading?"Loading the local spell catalog...":statusFilter==="all"?`No ${className} spells were found in the local catalog.`:"No spells match this collection filter."}/>} 
 </section>;
}

const numeric=(value:string)=>{const parsed=Number.parseFloat(value);return Number.isFinite(parsed)?parsed:Number.MAX_SAFE_INTEGER};
const seconds=(value:string)=>value?`${value.replace(/\s*s(?:ec(?:onds?)?)?$/i,"")}s`:"-";
const percent=(value:number,total:number)=>total?`${Math.round(value/total*100)}%`:"0%";
const description=(row:CharacterSpellRow)=>row.info.description||row.info.effects.map(effect=>effect.description).join("; ")||"-";
