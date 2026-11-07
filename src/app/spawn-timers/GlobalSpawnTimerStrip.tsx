import {useCallback,useEffect,useMemo,useState} from "react";
import {listen} from "@tauri-apps/api/event";
import {getSpawnTimerSnapshot} from "../../shared/backend";
import type {SpawnTimerSnapshot} from "../../shared/contracts";
import {openSpawnTimerWindow} from "./SpawnTimerWindow";
import {spawnTimerCategory,spawnTimerState,spawnWindowRemainingPercent} from "./timerDisplay";

const EMPTY:SpawnTimerSnapshot={generatedAt:"",lingerSeconds:240,timers:[],globalTimers:[],catalog:{zoneCount:0,overrideCount:0,aliasCount:0,upstreamRevision:""}};
const parsedTime=(value?:string)=>value?Date.parse(value):Number.POSITIVE_INFINITY;
const HIDDEN_TIMERS_KEY="global-spawn-timers-hidden";
const sessionHiddenTimers=()=>{
 try{return new Set<string>((JSON.parse(sessionStorage.getItem(HIDDEN_TIMERS_KEY)||"[]") as Array<string|number>).map(String))}catch{return new Set<string>()}
};
const countdown=(milliseconds:number)=>{
 if(!Number.isFinite(milliseconds))return "Timing unknown";
 if(milliseconds<=0)return "Respawn due";
 const seconds=Math.ceil(milliseconds/1000),hours=Math.floor(seconds/3600),minutes=Math.floor((seconds%3600)/60),remaining=seconds%60;
 if(hours)return `${hours}h ${minutes}m ${remaining}s`;
 if(minutes)return `${minutes}m ${remaining}s`;
 return `${remaining}s`;
};
type StripTimer={id:string;name:string;detail:string;dueAt?:string;category:"global"|"window"|"zone";uncertain:boolean;inWindow?:boolean;windowPercent?:number};

export function GlobalSpawnTimerStrip(){
 const[data,setData]=useState<SpawnTimerSnapshot>(EMPTY),[hidden,setHidden]=useState<Set<string>>(sessionHiddenTimers),[now,setNow]=useState(Date.now());
 const refresh=useCallback(()=>getSpawnTimerSnapshot().then(setData).catch(()=>{}),[]);
 useEffect(()=>{void refresh();const poll=window.setInterval(refresh,30000),clock=window.setInterval(()=>setNow(Date.now()),1000);let unlistenLive:(()=>void)|undefined,unlistenData:(()=>void)|undefined;if("__TAURI_INTERNALS__" in window){listen<string[]>("live-log-changed",event=>{if(event.payload.includes("spawnTimers"))void refresh()}).then(value=>unlistenLive=value);listen<string>("data-changed",event=>{if(event.payload==="spawn-timer.anchor"||event.payload.startsWith("spawnTimer."))void refresh()}).then(value=>unlistenData=value)}return()=>{window.clearInterval(poll);window.clearInterval(clock);unlistenLive?.();unlistenData?.()}},[refresh]);
 const visible=useMemo<StripTimer[]>(()=>[
  ...data.globalTimers.map(timer=>({id:`global:${timer.id}`,name:timer.name,detail:`${timer.server} · ${timer.origin}`,dueAt:timer.dueAt,category:"global" as const,uncertain:timer.uncertain})),
  ...data.timers.map(timer=>{const state=spawnTimerState(timer,now);return{id:`mob:${timer.id}`,name:timer.mobName,detail:`${timer.zoneName||"Unknown zone"}${timer.originKind==="manualSpawn"?" · manual anchor":""}${timer.totalWindows>1?` · ${state.windowLabel}`:""}`,dueAt:new Date(state.target).toISOString(),category:spawnTimerCategory(timer),uncertain:false,inWindow:state.phase==="window",windowPercent:spawnWindowRemainingPercent(timer,now)}}),
 ].filter(timer=>!hidden.has(timer.id)).sort((left,right)=>parsedTime(left.dueAt)-parsedTime(right.dueAt)),[data,hidden,now]);
 const groups=(["global","window","zone"] as const).map(category=>({category,timers:visible.filter(timer=>timer.category===category)})).filter(group=>group.timers.length);
 const dismiss=(id:string)=>setHidden(current=>{const next=new Set(current).add(id);sessionStorage.setItem(HIDDEN_TIMERS_KEY,JSON.stringify([...next]));return next});
 if(!visible.length)return null;
 return <section className="global-spawn-timers" aria-label="Mob respawn timers">
  <header><div><span className="spawn-timer-dot"/><strong>Spawn timers</strong><small>{visible.length} tracked</small></div><div className="global-spawn-actions"><button onClick={()=>void openSpawnTimerWindow()}>Undock</button><a href="#/spawn-timers">Open timers</a></div></header>
  <div className="global-spawn-timer-groups">{groups.map(group=><div className={`global-spawn-timer-group ${group.category}`} key={group.category}><span>{group.category==="global"?"Global":group.category==="window"?"Windows":"Zone"}</span><div className="global-spawn-timer-cards">{group.timers.map(timer=><GlobalSpawnTimerCard key={timer.id} timer={timer} now={now} dismiss={()=>dismiss(timer.id)}/>)}</div></div>)}</div>
 </section>;
}

function GlobalSpawnTimerCard({timer,now,dismiss}:{timer:StripTimer;now:number;dismiss:()=>void}){
 const remaining=parsedTime(timer.dueAt)-now,due=remaining<=0;
 return <article className={`${due?"due":""} ${timer.category}`}>
  <div><strong title={timer.name}>{timer.name}</strong><small title={timer.detail}>{timer.detail}</small></div>
  <time className={timer.uncertain?"uncertain":""} dateTime={timer.dueAt}>{timer.category==="window"?(timer.inWindow?`IN WINDOW · ${countdown(remaining).replace("Respawn due","closing")}`:`OPENS IN ${countdown(remaining)}`):countdown(remaining)}</time>
  <button type="button" title="Clear from workspace" aria-label={`Clear ${timer.name} from workspace`} onClick={dismiss}>×</button>
  {timer.windowPercent!==undefined&&<span className="global-window-progress" title={`${Math.round(timer.windowPercent)}% of the spawn window remains`}><i style={{width:`${timer.windowPercent}%`}}/></span>}
 </article>;
}
