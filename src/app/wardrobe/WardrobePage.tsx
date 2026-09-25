import {useEffect,useMemo,useRef,useState} from "react";
import type {AppSnapshot,WardrobeCatalogItem,WardrobeSetSummary} from "../../shared/contracts";
import {getWardrobeCatalogItems,getWardrobeCatalogSets,getWardrobeSetItems} from "../../shared/backend";
import {DataTable,Modal,money,type Column} from "../ui";
import {ItemHover} from "../items/ItemHover";
import {EquipmentPaperDollView,itemIconPath,PAPERDOLL_SLOT_GROUPS,type EquipmentSlot,type EquipmentSlotKey} from "../characters/EquipmentPaperDoll";
import {CHARACTER_CLASSES,characterClassesForRace,characterRacesForClass,isCharacterRaceClassCompatible,type CharacterModelProfile} from "../characters/characterProfile";
import {classBit,DEFAULT_WARDROBE_DRAFT,filterByStats,filterByTradeability,filterByWeaponHandedness,raceBit,SLOT_BITS,sortByWardrobeStats,statLabel,wardrobeItemToInventory,wardrobeTotals,WARDROBE_CHOOSER_STATS,WARDROBE_STATS,type WardrobeChooserStatKey,type WardrobeDraft,type WardrobeSortDirection,type WardrobeStatFilter,type WardrobeStatKey,type WardrobeStatOperator,type WardrobeStatSort,type WardrobeTradeability,type WeaponHandedness} from "./model";
import {WARDROBE_STORAGE_KEY} from "./characterTransfer";

const allSlots=PAPERDOLL_SLOT_GROUPS.flat();
const slotByKey=new Map(allSlots.map(slot=>[slot.key,slot]));
const SET_SLOT_KEYS:Record<string,EquipmentSlotKey[]>={
 head:["head"],face:["face"],neck:["neck"],shoulders:["shoulders"],arms:["arms"],back:["back"],
 wrist1:["left-wrist","right-wrist"],range:["range"],hands:["hands"],primary:["primary"],secondary:["secondary"],
 finger1:["left-finger","right-finger"],chest:["chest"],legs:["legs"],feet:["feet"],waist:["waist"],ammo:["ammo"],
};
const classNameByAbbreviation=new Map(CHARACTER_CLASSES.filter(entry=>entry.code).map(entry=>[entry.code.toUpperCase(),entry.name]));
const classSetHint=(set?:WardrobeSetSummary)=>set?.classes.length?set.classes.map(value=>classNameByAbbreviation.get(value)||value).join(", "):"Any class";

function loadDraft():WardrobeDraft{
 try{const parsed=JSON.parse(localStorage.getItem(WARDROBE_STORAGE_KEY)||"") as WardrobeDraft;return parsed?.profile&&parsed?.items?parsed:DEFAULT_WARDROBE_DRAFT}catch{return DEFAULT_WARDROBE_DRAFT}
}

export function WardrobePage({data}:{data:AppSnapshot}){
 const[draft,setDraft]=useState<WardrobeDraft>(loadDraft);
 const[pickerSlot,setPickerSlot]=useState<EquipmentSlot|null>(null);
 const[catalog,setCatalog]=useState<WardrobeCatalogItem[]>([]);
 const[sets,setSets]=useState<WardrobeSetSummary[]>([]);
 const[setChoice,setSetChoice]=useState("");
 const[pickerSet,setPickerSet]=useState("");
 const[setMessage,setSetMessage]=useState("");
 const[applyingSet,setApplyingSet]=useState(false);
 const[loading,setLoading]=useState(false);
 const[error,setError]=useState("");
 const[usableOnly,setUsableOnly]=useState(true);
 const[filters,setFilters]=useState<WardrobeStatFilter[]>([]);
 const[handedness,setHandedness]=useState<WeaponHandedness>("all");
 const[tradeability,setTradeability]=useState<WardrobeTradeability>("all");
 const[filterKey,setFilterKey]=useState<WardrobeStatKey>("ac");
 const[filterOperator,setFilterOperator]=useState<WardrobeStatOperator>(">=");
 const[filterValue,setFilterValue]=useState("1");
 const[sortKey,setSortKey]=useState<WardrobeChooserStatKey>("ac"),[sortDirection,setSortDirection]=useState<WardrobeSortDirection>("desc"),[sorts,setSorts]=useState<WardrobeStatSort[]>([]);
 const requestId=useRef(0);
 useEffect(()=>localStorage.setItem(WARDROBE_STORAGE_KEY,JSON.stringify(draft)),[draft]);
 useEffect(()=>{getWardrobeCatalogSets().then(setSets).catch(value=>setSetMessage(`Set catalog unavailable: ${String(value)}`))},[]);
 const selected=Object.entries(draft.items).filter((entry):entry is [EquipmentSlotKey,WardrobeCatalogItem]=>Boolean(entry[1]));
 const inventory=selected.map(([key,item],index)=>wardrobeItemToInventory(item,slotByKey.get(key)!,index));
 const totals=wardrobeTotals(selected.map(([,item])=>item));
 const masterById=useMemo(()=>new Map(data.items.map(item=>[item.id,item])),[data.items]);
 const masterByName=useMemo(()=>new Map(data.items.map(item=>[item.name.trim().toLowerCase(),item])),[data.items]);
 const market=(item:WardrobeCatalogItem)=>masterById.get(item.peqId||item.id)||masterByName.get(item.name.trim().toLowerCase());
 const slotSets=useMemo(()=>Array.from(new Set(catalog.flatMap(item=>item.setNames))).sort((left,right)=>left.localeCompare(right)),[catalog]);
 const selectedSet=sets.find(set=>set.name===setChoice);
 const selectedClassAbbreviation=draft.profile.classCode.toUpperCase();
 const selectedSetMatchesClass=Boolean(selectedSet&&(!selectedSet.classes.length||selectedSet.classes.includes(selectedClassAbbreviation)));
 const visible=useMemo(()=>sortByWardrobeStats(filterByStats(filterByTradeability(filterByWeaponHandedness(catalog,handedness),tradeability),filters).filter(item=>!pickerSet||item.setNames.includes(pickerSet)),sorts),[catalog,filters,handedness,pickerSet,sorts,tradeability]);
 const loadCandidates=async(slot:EquipmentSlot,profile:CharacterModelProfile,usable:boolean)=>{
  const id=++requestId.current;setLoading(true);setError("");setCatalog([]);
  try{const rows=await getWardrobeCatalogItems(SLOT_BITS[slot.key],usable?classBit(profile.classCode):0,usable?raceBit(profile.race):0);if(id===requestId.current)setCatalog(rows)}
  catch(value){if(id===requestId.current)setError(String(value))}finally{if(id===requestId.current)setLoading(false)}
 };
 const openSlot=(slot:EquipmentSlot)=>{setPickerSlot(slot);setFilters([]);setHandedness("all");setTradeability("all");setPickerSet("");loadCandidates(slot,draft.profile,usableOnly)};
 const updateProfile=(profile:CharacterModelProfile)=>{setDraft(current=>({...current,profile}));if(pickerSlot)loadCandidates(pickerSlot,profile,usableOnly)};
 const changeRace=(race:string)=>updateProfile({...draft.profile,race,classCode:isCharacterRaceClassCompatible(race,draft.profile.classCode)?draft.profile.classCode:""});
 const resetRaceClass=()=>updateProfile({...draft.profile,race:"hu",classCode:""});
 const changeUsable=(value:boolean)=>{setUsableOnly(value);if(pickerSlot)loadCandidates(pickerSlot,draft.profile,value)};
 const equip=(item:WardrobeCatalogItem)=>{if(!pickerSlot)return;setDraft(current=>({...current,items:{...current.items,[pickerSlot.key]:item}}));setPickerSlot(null)};
 const clearSlot=()=>{if(!pickerSlot)return;setDraft(current=>{const items={...current.items};delete items[pickerSlot.key];return{...current,items}});setPickerSlot(null)};
 const addFilter=()=>{const value=Number(filterValue);if(!Number.isFinite(value))return;setFilters(current=>[...current.filter(filter=>filter.key!==filterKey),{key:filterKey,operator:filterOperator,value}])};
 const addSort=()=>setSorts(current=>{const index=current.findIndex(sort=>sort.key===sortKey);if(index<0)return[...current,{key:sortKey,direction:sortDirection}];const next=[...current];next[index]={key:sortKey,direction:sortDirection};return next});
 const applySet=async()=>{
  if(!setChoice)return;
  setApplyingSet(true);setSetMessage("");
  try{
   const pieces=await getWardrobeSetItems(setChoice,classBit(draft.profile.classCode),raceBit(draft.profile.race));
   if(!pieces.length){setSetMessage(`No ${setChoice} pieces are usable by this race and class.`);return}
   setDraft(current=>{const items={...current.items};for(const piece of pieces)for(const key of SET_SLOT_KEYS[piece.slot]||[])items[key]=piece.item;return{...current,items}});
   setSetMessage(`Equipped ${pieces.length} compatible pieces from ${setChoice}.`);
  }catch(value){setSetMessage(`Could not equip set: ${String(value)}`)}finally{setApplyingSet(false)}
 };
 const columns:Column<WardrobeCatalogItem>[]=[
  {key:"name",label:"Item",value:item=>item.name,render:item=><ItemHover itemName={item.name} itemId={item.peqId||item.id} iconId={item.iconId} valuePp={market(item)?.valuePp} details={item} focusable={false}><span className="wardrobe-item-cell">{itemIconPath(item.iconId)?<img src={itemIconPath(item.iconId)} alt="" loading="lazy"/>:<i>?</i>}<span><strong>{item.name}</strong><small>ID {item.peqId||item.id} / WT {item.weight}</small></span></span></ItemHover>},
  {key:"set",label:"Set",value:item=>item.setNames.join(" "),render:item=>item.setNames.length?<span className="wardrobe-set-names">{item.setNames.join(", ")}</span>:"-"},
  {key:"ac",label:"AC",value:item=>item.ac},{key:"hp",label:"HP",value:item=>item.hp},{key:"mana",label:"Mana",value:item=>item.mana},
  {key:"stats",label:"Stats",value:item=>[item.strength,item.stamina,item.agility,item.dexterity,item.intelligence,item.wisdom,item.charisma,item.attack,item.haste,item.manaRegen,item.damageShield].join("/"),render:item=><span className="wardrobe-stat-stack">STR {item.strength} / STA {item.stamina} / AGI {item.agility}<small>DEX {item.dexterity} / INT {item.intelligence} / WIS {item.wisdom} / CHA {item.charisma}</small>{[item.attack,item.haste,item.manaRegen,item.damageShield].some(Boolean)&&<small>ATK {item.attack} / HST {item.haste} / MREG {item.manaRegen} / DS {item.damageShield}</small>}</span>},
  {key:"resists",label:"Resists",value:item=>[item.magicResist,item.fireResist,item.coldResist,item.diseaseResist,item.poisonResist].join("/"),render:item=><span className="wardrobe-stat-stack">MR {item.magicResist} / FR {item.fireResist} / CR {item.coldResist}<small>DR {item.diseaseResist} / PR {item.poisonResist}</small></span>},
  {key:"weapon",label:"Wpn",value:item=>[item.damage,item.delay].join("/"),render:item=>item.damage?<span>{item.damage} / {item.delay}<small className="wardrobe-substat">RAT {(item.damage/Math.max(1,item.delay)).toFixed(2)}</small></span>:"-"},
  {key:"effects",label:"FX",value:item=>[item.procName,item.clickName,item.wornName,item.focusName].filter(Boolean).join(" "),render:item=><span className="wardrobe-effects">{item.procName&&<small>Proc: {item.procName}</small>}{item.clickName&&<small>Click: {item.clickName}</small>}{item.wornName&&<small>Worn: {item.wornName}</small>}{item.focusName&&<small>Focus: {item.focusName}</small>}{![item.procName,item.clickName,item.wornName,item.focusName].some(Boolean)&&"-"}</span>},
  {key:"market",label:"PP",value:item=>market(item)?.valuePp||0,render:item=>money(market(item)?.valuePp)},
 ];
 return <section className="wardrobe-page">
  <section className="wardrobe-toolbar card"><header><div><span className="eyebrow">Appearance sandbox</span><h2>Build a wardrobe</h2><p>Choose a profile, equip a complete set, or click any equipment slot to build item by item.</p></div><button disabled={!selected.length} onClick={()=>setDraft(current=>({...current,items:{}}))}>Clear outfit</button></header><div className="character-profile-editor">
   <label><span>Race</span><select value={draft.profile.race} onChange={event=>changeRace(event.target.value)}>{characterRacesForClass(draft.profile.classCode).map(race=><option key={race.code} value={race.code}>{race.name}</option>)}</select></label>
   <label><span>Class</span><select value={draft.profile.classCode} onChange={event=>updateProfile({...draft.profile,classCode:event.target.value})}>{characterClassesForRace(draft.profile.race).map(entry=><option key={entry.code||"any"} value={entry.code}>{entry.code?entry.name:"Any class"}</option>)}</select></label>
   <label><span>Gender</span><select value={draft.profile.gender} onChange={event=>updateProfile({...draft.profile,gender:event.target.value as "m"|"f"})}><option value="m">Male</option><option value="f">Female</option></select></label>
   <button type="button" onClick={resetRaceClass}>Reset race / class</button><small>{draft.profile.classCode?"Outfit changes are saved automatically on this computer.":"Any class unlocks every race; choose a race, then narrow to a compatible class."}</small>
  </div><div className="wardrobe-set-control"><label><span>Equipment set</span><select value={setChoice} onChange={event=>{setSetChoice(event.target.value);setSetMessage("")}}><option value="">Choose from {sets.length||"…"} sets</option>{sets.map(set=><option key={set.name} value={set.name}>{set.name} · {set.itemCount} pieces · {set.classes.length?set.classes.join("/"):"ALL"}</option>)}</select></label><button disabled={!setChoice||!draft.profile.classCode||applyingSet} onClick={applySet}>{applyingSet?"Equipping…":"Equip set"}</button>{selectedSet&&<div className={`wardrobe-set-hint ${!draft.profile.classCode?"neutral":selectedSetMatchesClass?"compatible":"incompatible"}`}><strong>{!draft.profile.classCode?"Choose a class":selectedSetMatchesClass?"Class match":"Class mismatch"}</strong><span>Usable by: {classSetHint(selectedSet)}</span><small>Individual pieces are also checked against the selected race before equipping.</small></div>}{setMessage&&<span className="wardrobe-set-result" role="status">{setMessage}</span>}</div></section>
  <div className="wardrobe-workspace"><section className="card wardrobe-doll"><header><div><h2>Model &amp; equipment</h2><p>Occupied and empty slots are both selectable.</p></div><strong>{selected.length} / 21</strong></header><div className="wardrobe-doll-body"><EquipmentPaperDollView items={inventory} character="Wardrobe preview" profile={draft.profile} onSlotClick={openSlot} activeSlot={pickerSlot?.key}/></div></section>
   <aside className="wardrobe-summary card"><header><div><span className="eyebrow">Equipped totals</span><h2>Stat summary</h2></div></header><div className="wardrobe-total-grid">{WARDROBE_STATS.filter(stat=>!["damage","delay"].includes(stat.key)).map(stat=><article key={stat.key}><span>{stat.label}</span><strong>{totals[stat.key]||0}</strong></article>)}</div></aside>
  </div>
  {pickerSlot&&<Modal title={`Choose ${pickerSlot.label}`} onClose={()=>setPickerSlot(null)} footer={<><button disabled={!draft.items[pickerSlot.key]} onClick={clearSlot}>Clear slot</button><button onClick={()=>setPickerSlot(null)}>Close</button></>}><div className="wardrobe-picker">
   <section className="wardrobe-refine" aria-label="Filter and sort items">
    <div className="wardrobe-refine-toolbar"><label className="check-line"><input type="checkbox" checked={usableOnly} onChange={event=>changeUsable(event.target.checked)}/><span>Usable by selected race and class</span></label><label className="wardrobe-set-field"><span>Set</span><select value={pickerSet} onChange={event=>setPickerSet(event.target.value)}><option value="">All sets</option>{slotSets.map(name=><option key={name} value={name}>{name}</option>)}</select></label><label className="wardrobe-trade-field"><span>Trade</span><select value={tradeability} onChange={event=>setTradeability(event.target.value as WardrobeTradeability)}><option value="all">All items</option><option value="tradable">Tradable</option><option value="no-drop">NO DROP</option></select></label>{pickerSlot.key==="primary"&&<label className="wardrobe-weapon-field"><span>Weapon</span><select value={handedness} onChange={event=>setHandedness(event.target.value as WeaponHandedness)}><option value="all">1H + 2H</option><option value="1h">1H only</option><option value="2h">2H only</option></select></label>}<button className="wardrobe-reset-refine" disabled={!pickerSet&&handedness==="all"&&tradeability==="all"&&!filters.length&&!sorts.length} onClick={()=>{setPickerSet("");setHandedness("all");setTradeability("all");setFilters([]);setSorts([])}}>Reset all</button></div>
    <div className="wardrobe-refine-grid">
     <section className="wardrobe-refine-group"><header><div><span className="eyebrow">Narrow results</span><strong>Stat filters</strong></div><button disabled={!filters.length} onClick={()=>setFilters([])}>Clear</button></header><div className="wardrobe-filter-builder"><label><span>Stat</span><select value={filterKey} onChange={event=>setFilterKey(event.target.value as WardrobeStatKey)}>{WARDROBE_STATS.map(stat=><option key={stat.key} value={stat.key}>{stat.label}</option>)}</select></label><label className="wardrobe-operator"><span>Rule</span><select value={filterOperator} onChange={event=>setFilterOperator(event.target.value as WardrobeStatOperator)} aria-label="Comparison operator"><option value=">=">&gt;=</option><option value="<=">&lt;=</option><option value="=">=</option><option value=">">&gt;</option><option value="<">&lt;</option></select></label><label className="wardrobe-filter-value"><span>Value</span><input type="number" value={filterValue} onChange={event=>setFilterValue(event.target.value)} aria-label="Stat filter value"/></label><button className="wardrobe-add-rule" onClick={addFilter}>Add filter</button></div>{filters.length>0&&<div className="wardrobe-filter-chips">{filters.map(filter=><button key={filter.key} onClick={()=>setFilters(current=>current.filter(value=>value.key!==filter.key))} title="Remove filter"><span>{statLabel(filter.key)} {filter.operator} {filter.value}</span><b>&times;</b></button>)}</div>}</section>
     <section className="wardrobe-refine-group"><header><div><span className="eyebrow">Rank results</span><strong>Sort priorities</strong></div><button disabled={!sorts.length} onClick={()=>setSorts([])}>Clear</button></header><div className="wardrobe-sort-builder"><label><span>Stat</span><select value={sortKey} onChange={event=>setSortKey(event.target.value as WardrobeChooserStatKey)}>{WARDROBE_CHOOSER_STATS.map(stat=><option key={stat.key} value={stat.key}>{stat.label}</option>)}</select></label><label><span>Direction</span><select value={sortDirection} onChange={event=>setSortDirection(event.target.value as WardrobeSortDirection)}><option value="desc">High to low</option><option value="asc">Low to high</option></select></label><button className="wardrobe-add-rule" onClick={addSort}>Add priority</button></div>{sorts.length>0&&<div className="wardrobe-sort-chips" aria-label="Active sort priorities">{sorts.map((sort,index)=><button key={sort.key} onClick={()=>setSorts(current=>current.filter(value=>value.key!==sort.key))} title="Remove sort priority"><b>{index+1}</b><span>{WARDROBE_CHOOSER_STATS.find(stat=>stat.key===sort.key)?.label||sort.key} / {sort.direction==="desc"?"high to low":"low to high"}</span><em>&times;</em></button>)}</div>}</section>
    </div>
   </section>
   <div className="wardrobe-picker-status"><span>{loading?"Loading catalog…":`${visible.length} of ${catalog.length} compatible items · click a row to equip`}</span>{draft.items[pickerSlot.key]&&<strong>Equipped: {draft.items[pickerSlot.key]!.name}</strong>}</div>
   {error?<div className="alert"><strong>Catalog unavailable</strong><span>{error}</span><button onClick={()=>loadCandidates(pickerSlot,draft.profile,usableOnly)}>Retry</button></div>:loading?<div className="wardrobe-loading"><span className="spinner"/><p>Loading {pickerSlot.label.toLowerCase()} items…</p></div>:<DataTable rows={visible} columns={columns} rowKey={item=>item.id} empty="No items match all active filters." onRowClick={equip} rowClass={item=>draft.items[pickerSlot.key]?.id===item.id?"selected-row":""} sortMode="preserve"/>}
  </div></Modal>}
 </section>;
}
