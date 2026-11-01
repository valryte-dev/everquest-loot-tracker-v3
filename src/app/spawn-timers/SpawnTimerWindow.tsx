import {useCallback,useEffect,useMemo,useRef,useState} from "react";
import {listen} from "@tauri-apps/api/event";
import {getCurrentWindow} from "@tauri-apps/api/window";
import {WebviewWindow} from "@tauri-apps/api/webviewWindow";
import {getSpawnTimerSnapshot} from "../../shared/backend";
import type {SpawnTimerSnapshot} from "../../shared/contracts";
import {spawnTimerCategory,spawnTimerState,spawnWindowRemainingPercent} from "./timerDisplay";
import "./spawn-timer-window.css";

const EMPTY:SpawnTimerSnapshot={generatedAt:"",lingerSeconds:240,timers:[],globalTimers:[],catalog:{zoneCount:0,overrideCount:0,aliasCount:0,upstreamRevision:""}};
const isDesktop=()=>"__TAURI_INTERNALS__" in window;
const stamp=(value?:string)=>value?Date.parse(value):Number.POSITIVE_INFINITY;
const timerText=(dueAt:string|undefined,now:number)=>{
 if(!dueAt)return "Timing unknown";
 const value=Math.ceil((stamp(dueAt)-now)/1000);
 if(value<=0)return "Respawn due";
 const hours=Math.floor(value/3600),minutes=Math.floor((value%3600)/60),seconds=value%60;
 return `${hours?`${hours}h `:""}${hours||minutes?`${minutes}m `:""}${seconds}s`;
};

export async function openSpawnTimerWindow(){
 const width=Math.max(300,Math.min(900,Number(localStorage.getItem("spawn-timer-window-width"))||420));
 const height=Math.max(180,Math.min(1200,Number(localStorage.getItem("spawn-timer-window-height"))||460));
 if(!isDesktop()){window.open("?widget=spawn-timers","spawn-timers",`width=${width},height=${height},resizable=yes`);return}
 const existing=await WebviewWindow.getByLabel("spawn-timers");
 if(existing){await existing.show();await existing.setFocus();return}
 const widget=new WebviewWindow("spawn-timers",{url:"?widget=spawn-timers",title:"EQ Spawn Timers",width,height,minWidth:300,minHeight:180,resizable:true,alwaysOnTop:true,center:true});
 await new Promise<void>((resolve,reject)=>{void widget.once("tauri://created",()=>resolve());void widget.once("tauri://error",event=>reject(new Error(String(event.payload))))});
}

type CompactTimer={id:string;name:string;detail:string;dueAt?:string;category:"global"|"window"|"zone";uncertain:boolean;inWindow?:boolean;windowPercent?:number};

export function SpawnTimerWindow(){
 const[data,setData]=useState(EMPTY),[now,setNow]=useState(Date.now()),[onTop,setOnTop]=useState(true),[hidden,setHidden]=useState<Set<string>>(new Set()),[error,setError]=useState("");
 const refreshTimer=useRef<number|undefined>(undefined);
 const refresh=useCallback(()=>getSpawnTimerSnapshot().then(value=>{setData(value);setError("")}).catch(reason=>setError(String(reason))),[]);
 useEffect(()=>{void refresh();const poll=window.setInterval(refresh,30000),clock=window.setInterval(()=>setNow(Date.now()),1000);let stop:(()=>void)|undefined;if(isDesktop())void listen("data-changed",()=>{window.clearTimeout(refreshTimer.current);refreshTimer.current=window.setTimeout(()=>void refresh(),150)}).then(value=>stop=value);const remember=()=>{localStorage.setItem("spawn-timer-window-width",String(window.outerWidth));localStorage.setItem("spawn-timer-window-height",String(window.outerHeight))};window.addEventListener("resize",remember);return()=>{window.clearTimeout(refreshTimer.current);window.clearInterval(poll);window.clearInterval(clock);window.removeEventListener("resize",remember);stop?.()}},[refresh]);
 const timers=useMemo<CompactTimer[]>(()=>[
  ...data.globalTimers.map(timer=>({id:`global:${timer.id}`,name:timer.name,detail:`${timer.server} · ${timer.origin}`,dueAt:timer.dueAt,category:"global" as const,uncertain:timer.uncertain})),
  ...data.timers.map(timer=>{const state=spawnTimerState(timer,now);return{id:`mob:${timer.id}`,name:timer.mobName,detail:`${timer.zoneName||"Unknown zone"}${timer.originKind==="manualSpawn"?" · manual anchor":""}${timer.totalWindows>1?` · ${state.windowLabel}`:""}`,dueAt:new Date(state.target).toISOString(),category:spawnTimerCategory(timer),uncertain:false,inWindow:state.phase==="window",windowPercent:spawnWindowRemainingPercent(timer,now)}}),
 ].filter(timer=>!hidden.has(timer.id)).sort((left,right)=>stamp(left.dueAt)-stamp(right.dueAt)),[data,hidden,now]);
 const groups=(["global","window","zone"] as const).map(category=>({category,timers:timers.filter(timer=>timer.category===category)})).filter(group=>group.timers.length);
 const toggleTop=async()=>{const next=!onTop;if(isDesktop())await getCurrentWindow().setAlwaysOnTop(next);setOnTop(next)};
 return <main className="spawn-timer-window"><header><div><span>Always-visible widget</span><strong>EQ Spawn Timers</strong><small>{timers.length} timer{timers.length===1?"":"s"}</small></div><label><input type="checkbox" checked={onTop} onChange={()=>void toggleTop()}/> On top</label></header>
  {error&&<div className="spawn-window-error">{error}</div>}
  <section className="spawn-window-groups">{timers.length?groups.map(group=><div className={`spawn-window-group ${group.category}`} key={group.category}><h2>{group.category==="global"?"Global timers":group.category==="window"?"Window mobs":"Zone timers"}<small>{group.timers.length}</small></h2>{group.timers.map(timer=><article key={timer.id} className={timer.category}><i/><div><strong title={timer.name}>{timer.name}</strong><small title={timer.detail}>{timer.detail}</small></div><time className={timer.uncertain?"uncertain":""}>{timer.category==="window"?(timer.inWindow?`IN WINDOW · ${timerText(timer.dueAt,now)}`:`OPENS IN ${timerText(timer.dueAt,now)}`):timerText(timer.dueAt,now)}</time><button title="Hide from this window" aria-label={`Hide ${timer.name}`} onClick={()=>setHidden(current=>new Set(current).add(timer.id))}>×</button>{timer.windowPercent!==undefined&&<span className="spawn-window-progress" title={`${Math.round(timer.windowPercent)}% of the spawn window remains`}><i style={{width:`${timer.windowPercent}%`}}/></span>}</article>)}</div>):<div className="spawn-window-empty"><strong>No visible timers</strong><small>New confirmed kills and server timers appear automatically.</small></div>}</section>
 </main>;
}
