import {useCallback,useEffect,useMemo,useRef,useState} from "react";
import {listen} from "@tauri-apps/api/event";
import {getCurrentWindow} from "@tauri-apps/api/window";
import {WebviewWindow} from "@tauri-apps/api/webviewWindow";
import type {DamageEncounter,DamageEncounterDetail,GlobalStatusSnapshot} from "../../shared/contracts";
import {getDamageEncounterDetails,getGlobalStatus,getRevision} from "../../shared/backend";
import {formatCombatClock,participantCombatSeconds,playerSpellMetrics,rankIncomingTargets,rankMeterPlayers,summarizePlayerEffects} from "./meterModel";
import {sortLiveEncountersByPlayerTarget} from "./model";
import {DamageFighterBars,spellEffectTitle,type DamageFighterBarRow} from "./DamageFighterBars";
import {DamageIncomingBars,IncomingDamageTotal} from "./DamageIncomingBars";
import {DamageProcBars} from "./DamageProcBars";
import {TrackedSpellsPanel} from "./DamageSpellActivity";
import {DamageTrendCharts} from "./DamageTrendCharts";

const isDesktop=()=>"__TAURI_INTERNALS__" in window;
const stamp=(value:string)=>Date.parse(value.includes("T")?value:value.replace(" ","T"));
const formatNumber=(value:number)=>Math.round(value).toLocaleString();

export async function openDamageMeterWidget(){
 if(!isDesktop()){window.open("?widget=damage-meter","damage-meter","width=860,height=720");return}
 const existing=await WebviewWindow.getByLabel("damage-meter");
 if(existing){await existing.show();await existing.setFocus();return}
 const widget=new WebviewWindow("damage-meter",{
  url:"?widget=damage-meter",title:"EQ Damage Meter",width:860,height:720,
  minWidth:680,minHeight:420,resizable:true,alwaysOnTop:true,center:true,
 });
 await new Promise<void>((resolve,reject)=>{
  void widget.once("tauri://created",()=>resolve());
  void widget.once("tauri://error",event=>reject(new Error(String(event.payload))));
 });
}

type Activity={signature:string;seenAt:number};

export function DamageMeterWindow(){
 const[data,setData]=useState<GlobalStatusSnapshot|null>(null);
 const[error,setError]=useState("");
 const[now,setNow]=useState(Date.now());
 const[onTop,setOnTop]=useState(true);
 const[activity,setActivity]=useState<Map<number,Activity>>(()=>new Map());
 const revision=useRef(-1),refreshing=useRef(false),refreshQueued=useRef(false),refreshTimer=useRef<number|undefined>(undefined);
 const refresh=useCallback(async()=>{
  refreshQueued.current=true;
  if(refreshing.current)return;
  refreshing.current=true;
  try{
   while(refreshQueued.current){
    refreshQueued.current=false;
    const next=await getGlobalStatus();
    const current=Date.now();
    setData(next);
    setActivity(previous=>{
     const updated=new Map(previous);
     next.damageEncounters.forEach(row=>{
      const signature=[row.hitCount,row.totalDamage,row.incomingHitCount||0,row.incomingDamage||0,row.lastDamageAt,row.trackedSpells?.[0]?.id||0].join(":");
      const prior=updated.get(row.id);
      if(!prior||prior.signature!==signature){
       const recent=Math.abs(current-stamp(row.lastDamageAt))<=30000;
       updated.set(row.id,{signature,seenAt:prior||recent?current:0});
      }
     });
     return updated;
    });
    document.documentElement.dataset.theme=next.theme||"midnight";
    setError("");
   }
  }catch(reason){setError(String(reason))}
  finally{refreshing.current=false;if(refreshQueued.current)void refresh()}
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
  if(isDesktop())void listen("data-changed",()=>{window.clearTimeout(refreshTimer.current);refreshTimer.current=window.setTimeout(()=>void refresh(),100)}).then(unlisten=>stop=unlisten);
  return()=>{window.clearTimeout(refreshTimer.current);window.clearInterval(clock);window.clearInterval(guard);stop?.()}
 },[refresh]);
 const fights=useMemo(()=>{
  if(!data)return[];
  const character=data.activeCharacter?.toLowerCase();
  const filtered=data.damageEncounters.filter(row=>{
   const seenAt=activity.get(row.id)?.seenAt||0;
   return seenAt>0&&now-seenAt<=30000&&(!character||row.character.toLowerCase()===character);
  });
  const preferred=data.preferredTargetCharacter?.toLowerCase()===character?data.preferredTargetEncounterId:undefined;
  return sortLiveEncountersByPlayerTarget(filtered,character,preferred);
 },[activity,data,now]);
 const toggleTop=async()=>{
  const next=!onTop;
  if(isDesktop())await getCurrentWindow().setAlwaysOnTop(next);
  setOnTop(next);
 };
 return <main className="damage-meter-window">
  <header className="damage-meter-toolbar">
   <div><span>Live widget</span><strong>EQ Damage Meter</strong><small>{data?.activeCharacter||"Waiting for a character"}</small></div>
   <label className="meter-top-toggle"><input type="checkbox" checked={onTop} onChange={()=>void toggleTop()}/><span>On top</span></label>
  </header>
  {error&&<div className="meter-error">{error}</div>}
  {fights.length?<section className="damage-meter-fights">{fights.map(row=><DamageMeterPanel key={row.id} row={row} now={now}/>)}</section>:<section className="meter-waiting"><div><i/><strong>Listening for combat</strong><small>Each simultaneous encounter will appear as its own live damage card.</small></div></section>}
 </main>;
}

export function DamageMeterPanel({row,now,embedded=false}:{row:DamageEncounter;now:number;embedded?:boolean}){
 const[detail,setDetail]=useState<DamageEncounterDetail|null>(null);
 const previousShares=useRef(new Map<string,number>());
 const detailTimer=useRef<number|undefined>(undefined),detailRequestedAt=useRef(0);
 useEffect(()=>{
  let active=true;
  window.clearTimeout(detailTimer.current);
  const delay=Math.max(0,500-(Date.now()-detailRequestedAt.current));
  detailTimer.current=window.setTimeout(()=>{
   detailRequestedAt.current=Date.now();
   void getDamageEncounterDetails(row.id).then(value=>{if(active)setDetail(value)}).catch(()=>{});
  },delay);
  return()=>{active=false;window.clearTimeout(detailTimer.current)};
 },[row.id,row.hitCount,row.lastDamageAt]);
 const encounterEnd=row.outcome==="active"?now:stamp(row.lastDamageAt);
 const duration=Math.max(1,Math.floor((encounterEnd-stamp(row.startedAt))/1000));
 const players=rankMeterPlayers(row.players,row.totalDamage,duration);
 const incomingTargets=rankIncomingTargets(row.damageTargets||[]);
 const shares=new Map(previousShares.current);
 const effectsByPlayer=useMemo(()=>new Map(row.players.map(player=>{
  const metrics=playerSpellMetrics(row,player.name);
  return [player.name.toLowerCase(),{effects:summarizePlayerEffects(row,detail?.events||[],player.name),title:spellEffectTitle(metrics)}] as const;
 })),[row.players,row.spellMetrics,detail?.events]);
 useEffect(()=>{previousShares.current=new Map(players.map(player=>[player.name,player.contribution]))},[row.totalDamage,row.hitCount]);
 const names=players.slice(0,6).map(player=>player.name);
 const fighterRows:DamageFighterBarRow[]=players.map(player=>{
  const mine=player.name.toLowerCase()===row.character.toLowerCase();
  const prior=shares.get(player.name);
  const effects=effectsByPlayer.get(player.name.toLowerCase());
  return {
   name:player.name,rank:player.rank,totalDamage:player.totalDamage,dps:player.dps,
   combatSeconds:participantCombatSeconds(player,encounterEnd),contribution:player.contribution,
   contributionDelta:prior===undefined?0:player.contribution-prior,
   incomingDamage:row.damageTargets?.find(target=>target.name.toLowerCase()===player.name.toLowerCase())?.totalDamage||0,
   mine,combatTimeTitle:`In combat since first hit at ${player.firstDamageAt}`,
   effects:effects?.effects||summarizePlayerEffects(row,[],player.name),effectsTitle:effects?.title,
  };
 });
 return <article className={"meter-fight"+(embedded?" is-embedded":"")}>
  {!embedded&&<header>
   <div><span className="meter-pulse"><i/>{row.outcome==="active"?"Live":"Recent"}</span><h2 title={row.mobName}>{row.mobName}</h2><small>Combat {formatCombatClock(duration)} | {row.character}{row.weapons.length?" | Last known weapon snapshot: "+row.weapons.join(" / "):""}</small></div>
   <div className="meter-group-totals"><span>{(row.totalDamage/duration).toFixed(1)}</span><small>Group DPS</small><strong>{formatNumber(row.totalDamage)} DMG</strong><em>{row.procCount||0} PROC / {formatNumber(row.dotDamage||0)} DOT</em></div>
  </header>}
  <div className="meter-comparison-grid">
   <section className="meter-ranking-panel outgoing"><header><div><span>Outgoing</span><strong>Damage dealt</strong></div><b>{formatNumber(row.totalDamage)}</b></header><DamageFighterBars fighters={fighterRows}/></section>
   <section className="meter-ranking-panel incoming"><header><div><span>Incoming</span><strong>Damage taken</strong></div><IncomingDamageTotal totalDamage={row.incomingDamage||0} durationSeconds={duration}/></header><DamageIncomingBars targets={incomingTargets} activeCharacter={row.character}/></section>
  </div>
  <DamageProcBars metrics={row.spellMetrics||[]}/>
  {!embedded&&<TrackedSpellsPanel row={row}/>}
  <DamageTrendCharts events={detail?.events||[]} startedAt={row.startedAt} names={names} durationSeconds={duration}/>
 </article>;
}
