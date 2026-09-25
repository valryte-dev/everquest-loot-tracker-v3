import {useEffect,useRef,useState,type ReactNode} from "react";
import {createPortal} from "react-dom";
import {getWardrobeCatalogItem} from "../../shared/backend";
import type {WardrobeCatalogItem} from "../../shared/contracts";

type LoadState={kind:"idle"}|{kind:"loading"}|{kind:"ready";item:WardrobeCatalogItem|null}|{kind:"error"};
type Stat={label:string;value:number;signed?:boolean};

const cache=new Map<string,Promise<WardrobeCatalogItem|null>>();
const SLOT_LABELS:[number,string][]=[
 [2,"EAR"],[4,"HEAD"],[8,"FACE"],[16,"EAR"],[32,"NECK"],[64,"SHOULDERS"],[128,"ARMS"],[256,"BACK"],
 [512,"WRIST"],[1024,"WRIST"],[2048,"RANGE"],[4096,"HANDS"],[8192,"PRIMARY"],[16384,"SECONDARY"],
 [32768,"FINGER"],[65536,"FINGER"],[131072,"CHEST"],[262144,"LEGS"],[524288,"FEET"],[1048576,"WAIST"],[2097152,"AMMO"],
];
const CLASS_LABELS:[number,string][]=[[1,"WAR"],[2,"CLR"],[4,"PAL"],[8,"RNG"],[16,"SHD"],[32,"DRU"],[64,"MNK"],[128,"BRD"],[256,"ROG"],[512,"SHM"],[1024,"NEC"],[2048,"WIZ"],[4096,"MAG"],[8192,"ENC"]];
const RACE_LABELS:[number,string][]=[[1,"HUM"],[2,"BAR"],[4,"ERU"],[8,"ELF"],[16,"HIE"],[32,"DEF"],[64,"HEF"],[128,"DWF"],[256,"TRL"],[512,"OGR"],[1024,"HFL"],[2048,"GNM"],[4096,"IKS"]];

const labelsForMask=(mask:number,labels:[number,string][],allMask:number)=>mask===allMask?"ALL":labels.filter(([bit])=>(mask&bit)!==0).map(([,label])=>label).join(" ")||"None";
export const itemSlots=(mask:number)=>[...new Set(SLOT_LABELS.filter(([bit])=>(mask&bit)!==0).map(([,label])=>label))].join(" ")||"UNKNOWN";
export const itemClasses=(mask:number)=>labelsForMask(mask,CLASS_LABELS,16383);
export const itemRaces=(mask:number)=>labelsForMask(mask,RACE_LABELS,8191);
export function itemStats(item:WardrobeCatalogItem):{core:Stat[];attributes:Stat[];resists:Stat[];special:Stat[]}{
 const present=(rows:Stat[])=>rows.filter(row=>row.value!==0);
 return{
  core:present([{label:"AC",value:item.ac},{label:"HP",value:item.hp,signed:true},{label:"MANA",value:item.mana,signed:true}]),
  attributes:present([{label:"STR",value:item.strength,signed:true},{label:"STA",value:item.stamina,signed:true},{label:"AGI",value:item.agility,signed:true},{label:"DEX",value:item.dexterity,signed:true},{label:"INT",value:item.intelligence,signed:true},{label:"WIS",value:item.wisdom,signed:true},{label:"CHA",value:item.charisma,signed:true}]),
  resists:present([{label:"SV MAGIC",value:item.magicResist,signed:true},{label:"SV FIRE",value:item.fireResist,signed:true},{label:"SV COLD",value:item.coldResist,signed:true},{label:"SV DISEASE",value:item.diseaseResist,signed:true},{label:"SV POISON",value:item.poisonResist,signed:true}]),
  special:present([{label:"ATK",value:item.attack,signed:true},{label:"HASTE",value:item.haste,signed:true},{label:"MANA REGEN",value:item.manaRegen,signed:true},{label:"DAMAGE SHIELD",value:item.damageShield,signed:true}]),
 };
}

const requestItem=(itemId:number|undefined,itemName:string)=>{
 const key=itemId?`id:${itemId}`:`name:${itemName.trim().toLocaleLowerCase()}`;
 let request=cache.get(key);
 if(!request){request=getWardrobeCatalogItem(itemId,itemName);cache.set(key,request);request.catch(()=>cache.delete(key))}
 return request;
};
const iconPath=(iconId?:number)=>iconId&&iconId>0?`/item-icons/Item_${iconId}.png`:undefined;
const pp=(value?:number)=>value&&value>0?`${Math.round(value).toLocaleString()} pp`:"No market estimate";

export function ItemHover({itemName,itemId,iconId,valuePp,details,children,focusable=true}:{itemName:string;itemId?:number;iconId?:number;valuePp?:number;details?:WardrobeCatalogItem;children:ReactNode;focusable?:boolean}){
 const anchor=useRef<HTMLSpanElement>(null);
 const mounted=useRef(true);
 const[state,setState]=useState<LoadState>(details?{kind:"ready",item:details}:{kind:"idle"});
 const[visible,setVisible]=useState(false);
 const[position,setPosition]=useState({top:0,left:0});
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false}},[]);
 useEffect(()=>setState(details?{kind:"ready",item:details}:{kind:"idle"}),[details,itemId,itemName]);
 const show=()=>{
  const rect=anchor.current?.getBoundingClientRect();
  if(rect){const below=rect.bottom+8;const top=below+430<=window.innerHeight?below:Math.max(12,rect.top-438);setPosition({top,left:Math.max(12,Math.min(rect.left,window.innerWidth-412))})}
  setVisible(true);
  if(state.kind==="idle"&&!details){setState({kind:"loading"});requestItem(itemId,itemName).then(item=>{if(mounted.current)setState({kind:"ready",item})}).catch(()=>{if(mounted.current)setState({kind:"error"})})}
 };
 return <span ref={anchor} className="item-hover-anchor" tabIndex={focusable?0:undefined} onMouseEnter={show} onMouseLeave={()=>setVisible(false)} onFocus={show} onBlur={()=>setVisible(false)}>
  {children}
  {visible&&createPortal(<aside className="item-popover" style={position} role="tooltip"><ItemCard name={itemName} itemId={itemId} iconId={iconId} valuePp={valuePp} state={state}/></aside>,document.body)}
 </span>;
}

function ItemCard({name,itemId,iconId,valuePp,state}:{name:string;itemId?:number;iconId?:number;valuePp?:number;state:LoadState}){
 if(state.kind==="loading")return <div className="item-popover-status"><i/><span>Loading item details...</span></div>;
 const item=state.kind==="ready"?state.item:null;
 const icon=iconPath(item?.iconId||iconId);
 if(!item)return <div className="item-card item-card-basic"><header>{icon?<img src={icon} alt=""/>:<i>?</i>}<div><small>EverQuest item</small><strong>{name}</strong></div></header><footer><span>{itemId?`Item ID ${itemId}`:"Catalog details unavailable"}</span><b>{pp(valuePp)}</b></footer></div>;
 const stats=itemStats(item);
 const effects=[["Effect",item.wornName],["Focus",item.focusName],["Click",item.clickName],["Proc",item.procName]].filter((row):row is string[]=>Boolean(row[1]));
 const renderStats=(rows:Stat[])=><div className="item-stat-line">{rows.map(row=><span key={row.label}><b>{row.label}</b> {row.signed&&row.value>0?"+":""}{row.value}</span>)}</div>;
 return <div className="item-card">
  <header>{icon?<img src={icon} alt=""/>:<i>?</i>}<div><small>Project 1999 item</small><strong>{item.name}</strong><span>{item.setNames.length?item.setNames.join(" / "):"Equipment"}</span></div></header>
  <section className="item-primary-stats"><p><b>Slot:</b> {itemSlots(item.slots)}</p>{stats.core.length>0&&renderStats(stats.core)}{item.damage>0&&<div className="item-stat-line"><span><b>DMG</b> {item.damage}</span><span><b>Delay</b> {item.delay}</span><span><b>Ratio</b> {(item.damage/Math.max(1,item.delay)).toFixed(2)}</span></div>}</section>
  {(stats.attributes.length>0||stats.resists.length>0||stats.special.length>0)&&<section className="item-stat-groups">{stats.attributes.length>0&&renderStats(stats.attributes)}{stats.resists.length>0&&renderStats(stats.resists)}{stats.special.length>0&&renderStats(stats.special)}</section>}
  {effects.length>0&&<section className="item-effects">{effects.map(([label,value])=><p key={label}><b>{label}:</b> {value}</p>)}</section>}
  <section className="item-usable"><p><b>Class:</b> {itemClasses(item.classes)}</p><p><b>Race:</b> {itemRaces(item.races)}</p></section>
  <footer><span>ID {item.peqId||item.id} / WT {(item.weight/10).toFixed(1)}</span><b>{pp(valuePp)}</b></footer>
 </div>;
}
