import {useCallback,useEffect,useMemo,useRef,useState,type CSSProperties} from "react";
import {listen} from "@tauri-apps/api/event";
import {getCurrentWindow} from "@tauri-apps/api/window";
import {WebviewWindow} from "@tauri-apps/api/webviewWindow";
import type {AppSnapshot,DamageEncounter,DamageEncounterDetail} from "../../shared/contracts";
import {getDamageEncounterDetails,getPageSnapshot,getRevision} from "../../shared/backend";
import {buildMeterTrend,formatCombatClock,participantCombatSeconds,rankMeterPlayers} from "./meterModel";
import {sortLiveEncountersByPlayerTarget} from "./model";

const isDesktop=()=>"__TAURI_INTERNALS__" in window;
const stamp=(value:string)=>Date.parse(value.includes("T")?value:value.replace(" ","T"));
const formatNumber=(value:number)=>Math.round(value).toLocaleString();

export async function openDamageMeterWidget(){
 if(!isDesktop()){window.open("?widget=damage-meter","damage-meter","width=480,height=720");return}
 const existing=await WebviewWindow.getByLabel("damage-meter");
 if(existing){await existing.show();await existing.setFocus();return}
 const widget=new WebviewWindow("damage-meter",{
  url:"?widget=damage-meter",title:"EQ Damage Meter",width:480,height:720,
  minWidth:360,minHeight:420,resizable:true,alwaysOnTop:true,center:true,
 });
 await new Promise<void>((resolve,reject)=>{
  void widget.once("tauri://created",()=>resolve());
  void widget.once("tauri://error",event=>reject(new Error(String(event.payload))));
 });
}

type Activity={signature:string;seenAt:number};

export function DamageMeterWindow(){
 const[data,setData]=useState<AppSnapshot|null>(null);
 const[error,setError]=useState("");
 const[now,setNow]=useState(Date.now());
 const[onTop,setOnTop]=useState(true);
 const[activity,setActivity]=useState<Map<number,Activity>>(()=>new Map());
 const revision=useRef(-1),refreshing=useRef(false);
 const refresh=useCallback(async()=>{
  if(refreshing.current)return;
  refreshing.current=true;
  try{
   const next=await getPageSnapshot("damage");
   const current=Date.now();
   setData(next);
   setActivity(previous=>{
    const updated=new Map(previous);
    next.damageEncounters.forEach(row=>{
     const signature=[row.hitCount,row.totalDamage,row.incomingHitCount||0,row.incomingDamage||0,row.lastDamageAt].join(":");
     const prior=updated.get(row.id);
     if(!prior||prior.signature!==signature){
      const recent=Math.abs(current-stamp(row.lastDamageAt))<=30000;
      updated.set(row.id,{signature,seenAt:prior||recent?current:0});
     }
    });
    return updated;
   });
   document.documentElement.dataset.theme=next.settings.theme||"midnight";
   setError("");
  }catch(reason){setError(String(reason))}
  finally{refreshing.current=false}
 },[]);
 useEffect(()=>{
  void refresh();
  const clock=window.setInterval(()=>setNow(Date.now()),1000);
  const guard=window.setInterval(async()=>{
   try{
    const value=await getRevision();
    if(value!==revision.current){revision.current=value;void refresh()}
   }catch{}
  },2000);
  let stop:(()=>void)|undefined;
  if(isDesktop())void listen("data-changed",()=>void refresh()).then(unlisten=>stop=unlisten);
  return()=>{window.clearInterval(clock);window.clearInterval(guard);stop?.()}
 },[refresh]);
 const fights=useMemo(()=>{
  if(!data)return[];
  const character=data.settings.active_character?.toLowerCase();
  const filtered=data.damageEncounters.filter(row=>{
   const seenAt=activity.get(row.id)?.seenAt||0;
   return seenAt>0&&now-seenAt<=30000&&(!character||row.character.toLowerCase()===character);
  });
  return sortLiveEncountersByPlayerTarget(filtered,character);
 },[activity,data,now]);
 const toggleTop=async()=>{
  const next=!onTop;
  if(isDesktop())await getCurrentWindow().setAlwaysOnTop(next);
  setOnTop(next);
 };
 return <main className="damage-meter-window">
  <header className="damage-meter-toolbar">
   <div><span>Live widget</span><strong>EQ Damage Meter</strong><small>{data?.settings.active_character||"Waiting for a character"}</small></div>
   <label className="meter-top-toggle"><input type="checkbox" checked={onTop} onChange={()=>void toggleTop()}/><span>On top</span></label>
  </header>
  {error&&<div className="meter-error">{error}</div>}
  {fights.length?<section className="damage-meter-fights">{fights.map(row=><DamageMeterPanel key={row.id} row={row} now={now}/>)}</section>:<section className="meter-waiting"><div><i/><strong>Listening for combat</strong><small>Each simultaneous encounter will appear as its own live damage card.</small></div></section>}
 </main>;
}

export function DamageMeterPanel({row,now,embedded=false}:{row:DamageEncounter;now:number;embedded?:boolean}){
 const[detail,setDetail]=useState<DamageEncounterDetail|null>(null);
 const previousShares=useRef(new Map<string,number>());
 useEffect(()=>{
  let active=true;
  void getDamageEncounterDetails(row.id).then(value=>{if(active)setDetail(value)}).catch(()=>{});
  return()=>{active=false};
 },[row.id,row.hitCount,row.lastDamageAt]);
 const encounterEnd=row.outcome==="active"?now:stamp(row.lastDamageAt);
 const duration=Math.max(1,Math.floor((encounterEnd-stamp(row.startedAt))/1000));
 const players=rankMeterPlayers(row.players,row.totalDamage,duration);
 const maxDamage=Math.max(1,...players.map(player=>player.totalDamage));
 const shares=new Map(previousShares.current);
 useEffect(()=>{previousShares.current=new Map(players.map(player=>[player.name,player.contribution]))},[row.totalDamage,row.hitCount]);
 const colors=["#fb56a3","#a878fa","#57c7ff","#ffa357","#7bdfb2","#f37d8d"];
 const names=players.slice(0,6).map(player=>player.name);
 const trend=buildMeterTrend(detail?.events||[],row.startedAt,names);
 const maxTrend=Math.max(1,...trend.flatMap(point=>Object.values(point.totals)));
 const maxSecond=Math.max(1,trend.at(-1)?.second||1);
 const points=(name:string)=>trend.map(point=>(point.second/maxSecond*260)+","+(76-(point.totals[name]||0)/maxTrend*68)).join(" ");
 return <article className={"meter-fight"+(embedded?" is-embedded":"")}>
  {!embedded&&<header>
   <div><span className="meter-pulse"><i/>{row.outcome==="active"?"Live":"Recent"}</span><h2 title={row.mobName}>{row.mobName}</h2><small>Combat {formatCombatClock(duration)} | {row.character}{row.weapons.length?" | "+row.weapons.join(" / "):""}</small></div>
   <div className="meter-group-totals"><span>{(row.totalDamage/duration).toFixed(1)}</span><small>Group DPS</small><strong>{formatNumber(row.totalDamage)} DMG</strong></div>
  </header>}
  <div className="meter-players">{players.map((player,index)=>{
   const mine=player.name.toLowerCase()===row.character.toLowerCase();
   const prior=shares.get(player.name);
   const delta=prior===undefined?0:player.contribution-prior;
   const taken=row.damageTargets?.find(target=>target.name.toLowerCase()===player.name.toLowerCase())?.totalDamage||0;
   const fighterSeconds=participantCombatSeconds(player,encounterEnd);
   return <div key={player.name} className={"meter-player"+(mine?" is-me":"")} style={{"--meter-color":colors[index%colors.length]} as CSSProperties}>
    <div className="meter-contribution" style={{width:(player.totalDamage/maxDamage*100)+"%"}}/>
    <span className="meter-rank">#{player.rank}</span>
    <div className="meter-name"><strong><span title={player.name}>{player.name}</span>{mine&&<em>ME</em>}<time title={`In combat since first hit at ${player.firstDamageAt}`}>{formatCombatClock(fighterSeconds)}</time></strong><small>{formatNumber(taken)} damage taken</small></div>
    <div className="meter-share"><span>{delta>.05?"+":delta<-.05?"-":""} {player.contribution.toFixed(1)}%</span><small>share</small></div>
    <div className="meter-stat"><span>{player.dps.toFixed(1)}<small>DPS</small></span><strong>{formatNumber(player.totalDamage)}<small>DMG</small></strong></div>
   </div>;
  })}</div>
  {players.length>1&&trend.length>1&&<div className="meter-trend"><svg viewBox="0 0 260 80" role="img" aria-label="Cumulative damage plot">
   {[0,34,68].map(offset=><line key={offset} x1="0" x2="260" y1={76-offset} y2={76-offset}/>)}
   {players.slice(0,6).map((player,index)=><polyline key={player.name} style={{stroke:colors[index%colors.length]}} points={points(player.name)}/>)}
   <text x="2" y="78">0s</text><text x="258" y="78" textAnchor="end">{maxSecond}s</text>
  </svg></div>}
 </article>;
}
