import {useEffect,useRef,useState} from "react";
import {createPortal} from "react-dom";
import {openUrl} from "@tauri-apps/plugin-opener";
import {getSpellInfo} from "../shared/backend";
import type {SpellInfo} from "../shared/contracts";

type State={kind:"idle"}|{kind:"loading"}|{kind:"ready";info:SpellInfo}|{kind:"error";message:string};
const cache=new Map<string,Promise<SpellInfo>>();
const normalize=(value:string,force:boolean)=>{const match=value.trim().match(/^spell:\s*(.+?)\.?$/i);const name=match?.[1].trim()||(force?value.trim().replace(/\.$/,""):"");return name.replace(/[`‘’´]/g,"'")};
const load=(name:string)=>{const key=name.toLowerCase();let request=cache.get(key);if(!request){request=getSpellInfo(name);cache.set(key,request);request.catch(()=>cache.delete(key))}return request};

export function SpellHover({value,force=false}:{value:string;force?:boolean}){
 const name=normalize(value,force),anchor=useRef<HTMLSpanElement>(null),popover=useRef<HTMLElement>(null),suppressFocusPreview=useRef(false),[state,setState]=useState<State>({kind:"idle"}),[visible,setVisible]=useState(false),[pinned,setPinned]=useState(false),[position,setPosition]=useState({top:0,left:0});
 const close=(restoreFocus=true)=>{setPinned(false);setVisible(false);if(restoreFocus){suppressFocusPreview.current=true;requestAnimationFrame(()=>anchor.current?.focus({preventScroll:true}))}};
 useEffect(()=>{if(!pinned)return;const dismiss=(event:PointerEvent)=>{const target=event.target as Node;if(!anchor.current?.contains(target)&&!popover.current?.contains(target))close(false)};const escape=(event:KeyboardEvent)=>{if(event.key==="Escape")close()};document.addEventListener("pointerdown",dismiss);document.addEventListener("keydown",escape);return()=>{document.removeEventListener("pointerdown",dismiss);document.removeEventListener("keydown",escape)}},[pinned]);
 if(!name)return <>{value}</>;
 const show=()=>{const rect=anchor.current?.getBoundingClientRect();if(rect)setPosition({top:Math.max(12,Math.min(rect.bottom+8,window.innerHeight-390)),left:Math.max(12,Math.min(rect.left,window.innerWidth-440))});setVisible(true);if(state.kind==="idle"){setState({kind:"loading"});load(name).then(info=>setState({kind:"ready",info})).catch(error=>setState({kind:"error",message:String(error).replace(/^Error:\s*/,"")}))}};
 const pin=()=>{show();setPinned(true)};
 return <span ref={anchor} className={`spell-hover-anchor ${pinned?"pinned":""}`} tabIndex={0} role="button" aria-expanded={pinned} title="Hover to preview; click to pin" onClick={event=>{event.stopPropagation();pin()}} onKeyDown={event=>{if(event.key==="Enter"||event.key===" "){event.preventDefault();pin()}}} onMouseEnter={show} onMouseLeave={()=>{if(!pinned)setVisible(false)}} onFocus={()=>{if(suppressFocusPreview.current){suppressFocusPreview.current=false;return}show()}} onBlur={()=>{if(!pinned)setVisible(false)}}>{value}<span aria-hidden="true"> ✦</span>{visible&&createPortal(<SpellPopover popoverRef={popover} name={name} state={state} position={position} pinned={pinned} close={()=>close()}/>,document.body)}</span>;
}

function SpellPopover({popoverRef,name,state,position,pinned,close}:{popoverRef:React.Ref<HTMLElement>;name:string;state:State;position:{top:number;left:number};pinned:boolean;close:()=>void}){
 return <aside ref={popoverRef} className={`spell-popover ${pinned?"pinned":""}`} style={position} role={pinned?"dialog":"tooltip"} aria-label={pinned?`${name} spell details`:undefined} onClick={event=>event.stopPropagation()}>
  {pinned&&<div className="spell-popover-controls"><span>Pinned spell details</span><button onClick={close} aria-label="Close spell details" title="Close">×</button></div>}
  {state.kind==="loading"&&<div className="spell-popover-status"><i/><span>Loading {name} from the P99 wiki…</span></div>}
  {state.kind==="error"&&<div className="spell-popover-error"><strong>{name}</strong><span>{state.message}</span><small>Move away and hover again to retry.</small></div>}
  {state.kind==="ready"&&<SpellCard info={state.info} pinned={pinned}/>} 
 </aside>;
}

function SpellCard({info,pinned}:{info:SpellInfo;pinned:boolean}){
 const stats=[["Mana",info.mana],["Skill",info.skill],["Cast",info.castingTime&&`${info.castingTime}s`],["Recast",info.recastTime&&`${info.recastTime}s`],["Target",info.targetType],["Duration",info.duration],["Resist",info.resist],["Type",info.spellType]].filter((entry):entry is string[]=>Boolean(entry[1]));
 return <div className="spell-card">
  <header><div><small>Project 1999 spell</small><strong>{info.spellName}</strong></div>{info.stale&&<span>Cached</span>}</header>
  {info.description&&<p>{info.description}</p>}
  {!!info.classes.length&&<div className="spell-classes">{info.classes.map(row=><span key={row.name}>{row.name} <b>{row.level}</b></span>)}</div>}
  <dl>{stats.map(([label,value])=><div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl>
  {!!info.effects.length&&<section><small>Effects</small>{info.effects.map(effect=><p key={effect.slot}><b>{effect.slot}</b>{effect.description}</p>)}</section>}
  {(info.reagent||info.focus)&&<section className="spell-extras">{info.reagent&&<p><b>Reagent</b>{info.reagent}</p>}{info.focus&&<p><b>Focus</b>{info.focus}</p>}</section>}
  {(info.whereToObtain||pinned)&&<footer><div>{info.whereToObtain&&<><b>Obtained from</b><span>{info.whereToObtain}</span></>}</div>{pinned&&<button onClick={()=>openUrl(info.wikiUrl)}>Open wiki ↗</button>}</footer>}
 </div>;
}
