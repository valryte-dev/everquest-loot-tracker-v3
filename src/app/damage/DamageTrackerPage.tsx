import {useEffect,useMemo,useRef,useState} from "react";
import type {AppSnapshot,ClericHealCall,DamageEncounter,DamageEncounterDetail,DamageEvent,DamageParticipant} from "../../shared/contracts";
import {getDamageEncounterDetails} from "../../shared/backend";
import {DataTable,IconButton,Modal,when,type Column} from "../ui";
import {buildClericChainTimeline,buildDamageBurstSeries,buildLiveDpsSeries,dpsMomentum,latestHealChainBoundary,selectLiveEncounters} from "./model";

const time=(v:string)=>Date.parse(v.includes("T")?v:v.replace(" ","T"));
const seconds=(r:DamageEncounter)=>Math.max(0,Math.round((time(r.lastDamageAt)-time(r.startedAt))/1000));
const dps=(r:DamageEncounter)=>r.totalDamage/Math.max(1,seconds(r));
const own=(r:DamageEncounter)=>(r.players||[]).find(player=>player.name.toLowerCase()===r.character.toLowerCase());
const ownDamage=(r:DamageEncounter)=>own(r)?.totalDamage||0;
const ownDps=(r:DamageEncounter)=>ownDamage(r)/Math.max(1,seconds(r));
const num=(v:number)=>Math.round(v).toLocaleString();
const elapsed=(v:number)=>v<60?`${v}s`:`${Math.floor(v/60)}m ${v%60}s`;
const result=(v:DamageEncounter["outcome"])=>({active:"Open",slain:"Slain",playerDeath:"Player died",disengaged:"Disengaged"}[v]);
const filename=(v:string)=>v.split(/[\\/]/).pop()||v;

export function DamageTrackerPage({data,run}:{data:AppSnapshot;run:(a:string,p?:Record<string,unknown>)=>Promise<unknown>}){
 const[detail,setDetail]=useState<DamageEncounterDetail|null>(null),[loading,setLoading]=useState<number|null>(null),[error,setError]=useState(""),[scanning,setScanning]=useState(false);
 const[liveView,setLiveView]=useState<"compact"|"detail">(()=>["detail","previous"].includes(localStorage.getItem("damage-live-view")||"")?"detail":"compact");
 const rows=data.damageEncounters;
 const summary=useMemo(()=>rows.reduce((s,r)=>({damage:s.damage+r.totalDamage,myDamage:s.myDamage+ownDamage(r),hits:s.hits+r.hitCount,duration:s.duration+seconds(r)}),{damage:0,myDamage:0,hits:0,duration:0}),[rows]);
 const largest=useMemo(()=>[...rows].sort((a,b)=>b.totalDamage-a.totalDamage)[0],[rows]);
 const ranked=useMemo(()=>{const m=new Map<string,{damage:number,count:number}>();rows.forEach(r=>{const v=m.get(r.mobName)||{damage:0,count:0};v.damage+=r.totalDamage;v.count++;m.set(r.mobName,v)});return [...m].map(([mob,v])=>({mob,...v})).sort((a,b)=>b.damage-a.damage).slice(0,8)},[rows]),max=Math.max(1,...ranked.map(r=>r.damage));
 const columns:Column<DamageEncounter>[]=[
  {key:"time",label:"Engaged",value:r=>r.startedAt,render:r=>when(r.startedAt)},
  {key:"mob",label:"Mob",value:r=>r.mobName,render:r=><strong>{r.mobName}</strong>},
  {key:"character",label:"Character",value:r=>r.character},
  {key:"myDamage",label:"My Damage",value:ownDamage,render:r=><strong className="damage-mine">{num(ownDamage(r))}</strong>},
  {key:"myDps",label:"My DPS",value:ownDps,render:r=><strong className="damage-mine">{ownDps(r).toFixed(1)}</strong>},
  {key:"damage",label:"Group Damage",value:r=>r.totalDamage,render:r=><strong className="damage-total">{num(r.totalDamage)}</strong>},
  {key:"dps",label:"Group DPS",value:dps,render:r=>dps(r).toFixed(1)},
  {key:"duration",label:"Duration",value:seconds,render:r=>elapsed(seconds(r))},
  {key:"events",label:"Events",value:r=>r.hitCount},{key:"max",label:"Max Hit",value:r=>r.maxHit},
  {key:"weapons",label:"Weapons Used",value:r=>r.weapons.join(", "),render:r=><span className="damage-weapons">{r.weapons.join(" / ")||"Unknown"}</span>},
  {key:"mix",label:"Melee / Spell",value:r=>`${r.meleeDamage} ${r.spellDamage}`,render:r=><Mix row={r}/>},
  {key:"outcome",label:"Outcome",value:r=>result(r.outcome),render:r=><span className={`damage-outcome ${r.outcome}`}>{result(r.outcome)}</span>}
 ];
 const open=async(r:DamageEncounter)=>{setLoading(r.id);setError("");try{setDetail(await getDamageEncounterDetails(r.id))}catch(e){setError(String(e))}finally{setLoading(null)}};
 const scan=async()=>{setScanning(true);try{await run("damageTracker.rescan")}finally{setScanning(false)}};
 return <>
  <section className="damage-hero"><div><span className="eyebrow">Group combat telemetry</span><h2>Damage Tracker</h2><p>Observed player melee and spell damage, grouped into per-character encounters for each mob.</p></div><div className="damage-hero-actions"><div className="damage-view-toggle" aria-label="Live damage view"><button className={liveView==="compact"?"active":""} onClick={()=>{setLiveView("compact");localStorage.setItem("damage-live-view","compact")}}>Compact</button><button className={liveView==="detail"?"active":""} onClick={()=>{setLiveView("detail");localStorage.setItem("damage-live-view","detail")}}>Detail</button></div><IconButton icon="refresh" label="Scan all character logs" className="primary" disabled={scanning} onClick={scan}/></div></section>
  <LiveFightArea rows={rows} activeCharacter={data.settings.active_character} view={liveView} open={open}/>
  <CurrentPlayer character={data.settings.active_character} loadout={data.currentWeaponLoadout}/>
  <section className="damage-stats damage-stats-six">
   <article><span>Encounters</span><strong>{rows.length.toLocaleString()}</strong><small>{new Set(rows.map(r=>r.mobName.toLowerCase())).size} distinct mobs</small></article>
   <article className="mine"><span>My damage</span><strong>{num(summary.myDamage)}</strong><small>{Math.round(summary.myDamage/Math.max(1,summary.damage)*100)}% of group damage</small></article>
   <article className="mine"><span>My DPS</span><strong>{(summary.myDamage/Math.max(1,summary.duration)).toFixed(1)}</strong><small>Across encounter time</small></article>
   <article><span>Group damage</span><strong>{num(summary.damage)}</strong><small>{summary.hits.toLocaleString()} events</small></article>
   <article><span>Group DPS</span><strong>{(summary.damage/Math.max(1,summary.duration)).toFixed(1)}</strong><small>Across encounter time</small></article>
   <article><span>Largest encounter</span><strong>{largest?num(largest.totalDamage):"-"}</strong><small title={largest?.mobName}>{largest?.mobName||"Waiting for combat"}</small></article>
  </section>
  <ClericChainPanel calls={data.clericHealCalls||[]} encounters={rows} activeCharacter={data.settings.active_character} clearedAt={data.settings.ch_chain_cleared_at} run={run}/>
  <section className="damage-overview"><article className="card"><header><div><h2>Top mobs by damage</h2><p>Combined damage across all encounters.</p></div></header>{ranked.length?<div className="damage-ranking">{ranked.map((r,i)=><div key={r.mob}><b>{i+1}</b><span title={r.mob}>{r.mob}<small>{r.count} encounter{r.count===1?"":"s"}</small></span><i><em style={{width:`${r.damage/max*100}%`}}/></i><strong>{num(r.damage)}</strong></div>)}</div>:<div className="damage-empty">No combat damage discovered yet.</div>}</article><article className="card"><header><div><h2>Damage composition</h2><p>Melee compared with spell and non-melee damage.</p></div></header><Composition rows={rows}/></article></section>
  {error&&<div className="alert"><span>{error}</span><button onClick={()=>setError("")}>x</button></div>}
  <section className="card"><header><div><h2>Recorded encounters</h2><p>Filter or sort any column, then open a row for damage over time.</p></div><small>{data.settings.damage_tracker_last_scan_at?`Last scan ${when(data.settings.damage_tracker_last_scan_at)}`:"Initial scan pending"}</small></header><DataTable rows={rows} columns={columns} rowKey={r=>r.id} empty={scanning?"Scanning all logs...":"No damage encounters found yet."} actions={r=><IconButton icon="external" label={`View ${r.mobName} encounter`} disabled={loading===r.id} onClick={()=>open(r)}/>} /></section>
  {detail&&<Details row={detail} close={()=>setDetail(null)}/>}
 </>;
}

function CurrentPlayer({character,loadout}:{character?:string;loadout?:AppSnapshot["currentWeaponLoadout"]}){
 return <section className="damage-current-player"><div><span className="eyebrow">Current player</span><strong>{character||"Waiting for an active character"}</strong></div><div><span>Primary</span><strong>{loadout?.primary||"Empty / unknown"}</strong></div><div><span>Secondary</span><strong>{loadout?.secondary||"Empty / unknown"}</strong></div>{loadout?.capturedAt&&<small>Equipment captured {when(loadout.capturedAt)}</small>}</section>;
}

function ClericChainPanel({calls,encounters,activeCharacter,clearedAt,run}:{calls:ClericHealCall[];encounters:DamageEncounter[];activeCharacter?:string;clearedAt?:string;run:(a:string,p?:Record<string,unknown>)=>Promise<unknown>}){
 const[now,setNow]=useState(Date.now()),[pinned,setPinned]=useState(false);
 const flowRef=useRef<HTMLDivElement|null>(null);
 useEffect(()=>{const timer=window.setInterval(()=>setNow(Date.now()),1000);return()=>window.clearInterval(timer)},[]);
 useEffect(()=>setPinned(false),[activeCharacter]);
 const automaticBoundary=useMemo(()=>latestHealChainBoundary(encounters,activeCharacter,clearedAt),[encounters,activeCharacter,clearedAt]);
 const manualBoundary=(clearedAt?time(clearedAt):0)||0,boundary=pinned?manualBoundary:automaticBoundary;
 const scoped=useMemo(()=>calls.filter(call=>(!activeCharacter||call.character.toLowerCase()===activeCharacter.toLowerCase())&&time(call.happenedAt)>boundary),[calls,activeCharacter,boundary]);
 const allTimeline=useMemo(()=>buildClericChainTimeline(calls),[calls]);
 const timeline=useMemo(()=>buildClericChainTimeline(scoped),[scoped]),latestSession=timeline.at(-1)?.session;
 const candidate=latestSession===undefined?[]:timeline.filter(call=>call.session===latestSession),lastAt=candidate.length?time(candidate.at(-1)!.happenedAt):0;
 const current=pinned?candidate:lastAt&&now-lastAt<=15000?candidate:[];
 const gaps=current.flatMap(call=>call.gapSeconds===undefined?[]:[call.gapSeconds]),average=gaps.length?gaps.reduce((sum,gap)=>sum+gap,0)/gaps.length:0;
 const deviation=gaps.length?Math.sqrt(gaps.reduce((sum,gap)=>sum+(gap-average)**2,0)/gaps.length):0;
 const clerics=useMemo(()=>{const grouped=new Map<string,{name:string;calls:number;gaps:number[]}>();current.forEach(call=>{const key=call.clericName.toLowerCase(),row=grouped.get(key)||{name:call.clericName,calls:0,gaps:[]};row.calls++;if(call.clericGapSeconds!==undefined)row.gaps.push(call.clericGapSeconds);grouped.set(key,row)});return [...grouped.values()].sort((a,b)=>b.calls-a.calls||a.name.localeCompare(b.name))},[current]);
 const rotation=[...new Map(current.map(call=>[call.clericName.toLowerCase(),call.clericName])).values()],lastCleric=current.at(-1)?.clericName;
 const lastIndex=rotation.findIndex(name=>name.toLowerCase()===lastCleric?.toLowerCase()),nextCleric=rotation.length>1?rotation[(lastIndex+1)%rotation.length]:"Learning rotation";
 const currentGap=lastAt?Math.max(0,(now-lastAt)/1000):0,rows=[...allTimeline].reverse(),lastCallId=current.at(-1)?.id;
 useEffect(()=>{if(!lastCallId)return;const frame=window.requestAnimationFrame(()=>{const flow=flowRef.current;if(flow)flow.scrollTo({left:flow.scrollWidth,behavior:"smooth"})});return()=>window.cancelAnimationFrame(frame)},[lastCallId]);
 const columns:Column<(typeof rows)[number]>[]=[
  {key:"time",label:"Called",value:r=>r.happenedAt,render:r=>when(r.happenedAt)},
  {key:"cleric",label:"Cleric",value:r=>r.clericName,render:r=><strong>{r.clericName}</strong>},
  {key:"number",label:"Call",value:r=>r.callNumber,render:r=>"#"+String(r.callNumber).padStart(3,"0")},
  {key:"target",label:"Target",value:r=>r.targetName||"",render:r=>r.targetName||"-"},
  {key:"gap",label:"Chain Gap",value:r=>r.gapSeconds??-1,render:r=>r.gapSeconds===undefined?"New chain":r.gapSeconds.toFixed(1)+"s"},
  {key:"clericGap",label:"Cleric Rotation",value:r=>r.clericGapSeconds??-1,render:r=>r.clericGapSeconds===undefined?"-":r.clericGapSeconds.toFixed(1)+"s"},
  {key:"channel",label:"Channel",value:r=>r.channel},
  {key:"character",label:"Log Toon",value:r=>r.character},
 ];
 const visible=current.slice(-24),clear=()=>{setPinned(false);return run("setting.save",{key:"ch_chain_cleared_at",value:new Date().toISOString()})};
 return <section className="cleric-chain-stack"><article className={"card cleric-chain-live"+(pinned?" is-pinned":"")}><header><div><span className="eyebrow">Encounter support</span><h2>Complete Heal chain</h2><p>Calls auto-scroll as the chain advances. They clear after 15 seconds or a mob death unless pinned; history is retained.</p></div><div className="cleric-chain-actions"><strong>{current.length?current.length+" calls":"Waiting for guild LoF CH calls"}</strong><IconButton icon="pin" label={pinned?"Unpin CH encounter":"Pin CH encounter"} className={pinned?"active":""} disabled={!pinned&&!current.length} onClick={()=>setPinned(value=>!value)}/><IconButton icon="close" label="Clear current CH encounter" disabled={!current.length} onClick={clear}/></div></header>{current.length?<><div className="cleric-chain-stats"><div><span>Clerics</span><strong>{clerics.length}</strong></div><div><span>Average gap</span><strong>{average.toFixed(1)}s</strong></div><div><span>Longest gap</span><strong>{Math.max(0,...gaps).toFixed(1)}s</strong></div><div><span>Consistency</span><strong>+/- {deviation.toFixed(1)}s</strong></div><div><span>Last call</span><strong>#{String(current.at(-1)?.callNumber||0).padStart(3,"0")}</strong></div><div className="next"><span>Next expected</span><strong>{nextCleric}</strong><small>{currentGap.toFixed(1)}s current gap{average?" / "+average.toFixed(1)+"s typical":""}</small></div></div><div ref={flowRef} className="cleric-chain-flow">{visible.map((call,index)=><article key={call.id} className={call.gapSeconds!==undefined&&average&&Math.abs(call.gapSeconds-average)>Math.max(2,average*.25)?"variance":""}><div><b>#{String(call.callNumber).padStart(3,"0")}</b><span>{call.clericName}</span></div><strong>{call.gapSeconds===undefined?"Start":"+"+call.gapSeconds.toFixed(1)+"s"}</strong><small>{call.targetName?"on "+call.targetName:call.channel}</small>{index<visible.length-1&&<i/>}</article>)}</div><div className="cleric-performance">{clerics.map(cleric=><article key={cleric.name}><span>{cleric.name}</span><strong>{cleric.calls} call{cleric.calls===1?"":"s"}</strong><small>{cleric.gaps.length?(cleric.gaps.reduce((sum,gap)=>sum+gap,0)/cleric.gaps.length).toFixed(1)+"s average rotation":"First rotation call"}</small></article>)}</div></>:<div className="damage-empty">Listening for a new guild CH encounter. Previous guild calls remain in history below.</div>}</article><article className="card cleric-chain-history"><header><div><h2>CH call history</h2><p>Search or sort every captured call, chain gap, and per-cleric rotation interval.</p></div><small>{rows.length.toLocaleString()} calls</small></header><DataTable rows={rows} columns={columns} rowKey={row=>row.id} empty="No Complete Heal calls captured yet."/></article></section>;
}

function LiveFightArea({rows,activeCharacter,view,open}:{rows:DamageEncounter[];activeCharacter?:string;view:"compact"|"detail";open:(row:DamageEncounter)=>void}){
 const[now,setNow]=useState(Date.now()),[pinned,setPinned]=useState<number[]>([]),[closed,setClosed]=useState<number[]>([]);
 const observed=useRef(new Map<number,{signature:string;seenAt:number}>()),initialized=useRef(false);
 useEffect(()=>{const timer=window.setInterval(()=>setNow(Date.now()),1000);return()=>window.clearInterval(timer)},[]);
 useEffect(()=>{const stamp=Date.now(),initial=!initialized.current;rows.forEach(row=>{const signature=row.hitCount+":"+row.totalDamage+":"+(row.incomingHitCount||0)+":"+(row.incomingDamage||0)+":"+row.lastDamageAt,prior=observed.current.get(row.id);if(!prior||prior.signature!==signature){const logAge=Math.abs(stamp-time(row.lastDamageAt));observed.current.set(row.id,{signature,seenAt:prior||!initial||logAge<=30000?stamp:0})}});initialized.current=true},[rows]);
 const live=useMemo(()=>selectLiveEncounters(rows,activeCharacter,new Map([...observed.current].map(([id,value])=>[id,value.seenAt])),now,closed),[rows,activeCharacter,now,closed]);
 const cards=useMemo(()=>{const found:DamageEncounter[]=[...live];pinned.forEach(id=>{const row=rows.find(r=>r.id===id);if(row&&!closed.includes(id)&&!found.some(value=>value.id===id))found.push(row)});return found},[live,pinned,rows,closed]);
 return cards.length?<section className="damage-live-stack" aria-label="Live fights">{cards.map(row=><LiveFight key={row.id} row={row} now={now} seenAt={observed.current.get(row.id)?.seenAt||now} view={view} newest={live.some(value=>value.id===row.id)} pinned={pinned.includes(row.id)} pin={()=>setPinned(ids=>ids.includes(row.id)?ids:[...ids,row.id])} unpin={()=>setPinned(ids=>ids.filter(id=>id!==row.id))} close={()=>{setPinned(ids=>ids.filter(id=>id!==row.id));setClosed(ids=>ids.includes(row.id)?ids:[...ids,row.id])}} open={()=>open(row)}/>)}</section>:null;
}

function LiveFight({row,now,seenAt,view,newest,pinned,pin,unpin,close,open}:{row:DamageEncounter;now:number;seenAt:number;view:"compact"|"detail";newest:boolean;pinned:boolean;pin:()=>void;unpin:()=>void;close:()=>void;open:()=>void}){
 const idle=Math.max(0,Math.floor((now-seenAt)/1000)),finished=row.outcome!=="active"||idle>=30,duration=finished?Math.max(1,seconds(row)):Math.max(1,seconds(row)+idle),liveDps=row.totalDamage/duration,meleePct=row.totalDamage?row.meleeDamage/row.totalDamage*100:0,mine=own(row),myDamage=mine?.totalDamage||0,myDps=myDamage/duration,myShare=myDamage/Math.max(1,row.totalDamage)*100;
 return <article className={"damage-live "+(!pinned&&idle>=30?"is-fading ":"")+(pinned?"is-pinned":"")}>
  <header><div><span className="damage-live-pulse"><i/>{newest?"Current fight":"Pinned fight"}</span><h2>{row.character} <small>vs.</small> {row.mobName}<span className="damage-title-weapons">{row.weapons.join(" / ")||"Weapons unknown"}</span></h2><p>{idle===0?"Damage received now":idle+"s since last damage"} · {result(row.outcome)}</p></div><div className="damage-live-actions">{pinned?<><IconButton icon="pin" label="Unpin fight" onClick={unpin}/><IconButton icon="close" label="Close pinned fight" onClick={close}/></>:<IconButton icon="pin" label="Pin fight" onClick={pin}/>}<IconButton icon="external" label="Open fight details" onClick={open}/></div></header>
  <div className={"damage-live-metrics "+view}><div className="mine primary"><span>My DPS</span><strong>{myDps.toFixed(1)}</strong><small>{row.character} - {elapsed(duration)} elapsed</small></div><div className="mine"><span>My damage</span><strong>{num(myDamage)}</strong><small>{mine?.hitCount||0} events - {Math.round(myShare)}% share</small></div><div><span>Group DPS</span><strong>{liveDps.toFixed(1)}</strong><small>{elapsed(duration)} elapsed</small></div><div><span>Group damage</span><strong>{num(row.totalDamage)}</strong><small>{row.hitCount.toLocaleString()} events</small></div>{view==="detail"&&<><div><span>Group hits</span><strong>{row.hitCount.toLocaleString()}</strong></div><div><span>Group max hit</span><strong>{num(row.maxHit)}</strong></div></>}</div>
  <div className="damage-live-bar" title={"Melee "+num(row.meleeDamage)+" · Spell "+num(row.spellDamage)}><i style={{width:meleePct+"%"}}/><span>{Math.round(meleePct)}% melee · {Math.round(100-meleePct)}% spell</span></div>
  {view==="detail"&&<><LiveTelemetry row={row} duration={duration}/><PlayerCharts players={row.players}/><IncomingDamageChart row={row}/></>}
 </article>
}

function LiveTelemetry({row,duration}:{row:DamageEncounter;duration:number}){
 const[events,setEvents]=useState<DamageEvent[]>([]);
 useEffect(()=>{
  let cancelled=false;
  getDamageEncounterDetails(row.id).then(value=>{if(!cancelled)setEvents(value.events)}).catch(()=>{});
  return()=>{cancelled=true};
 },[row.id,row.hitCount,row.lastDamageAt]);
 const series=useMemo(()=>buildLiveDpsSeries(events,row.character,row.startedAt,duration),[events,row.character,row.startedAt,duration]);
 const bursts=useMemo(()=>buildDamageBurstSeries(events,row.character,row.startedAt,duration),[events,row.character,row.startedAt,duration]);
 const groupPeak=Math.max(0,...series.map(point=>point.group)),myPeak=Math.max(0,...series.map(point=>point.me)),burstPeak=Math.max(1,...bursts.map(point=>point.group)),latestBurst=bursts.at(-1)||{group:0,me:0},scale=Math.max(1,groupPeak,myPeak);
 const w=820,h=132,l=42,r=14,t=16,b=23,pw=w-l-r,ph=h-t-b;
 const x=(index:number)=>l+(series.length<=1?0:index/(series.length-1))*pw,y=(value:number)=>t+ph-value/scale*ph;
 const line=(key:"group"|"me")=>series.map((point,index)=>`${x(index)},${y(point[key])}`).join(" ");
 const groupPeakIndex=Math.max(0,series.findIndex(point=>point.group===groupPeak)),myPeakIndex=Math.max(0,series.findIndex(point=>point.me===myPeak));
 const groupMomentum=dpsMomentum(series,"group"),myMomentum=dpsMomentum(series,"me");
 return <section className="damage-live-telemetry">
  <article className="damage-live-rolling"><header><div><span className="eyebrow">Combat momentum</span><h3>30-second rolling DPS</h3></div><div className="damage-live-legend"><span className="group">Group <b>{groupPeak.toFixed(1)} peak</b></span><span className="me">Me <b>{myPeak.toFixed(1)} peak</b></span></div></header>
   {events.length?<div className="damage-live-chart"><svg viewBox={`0 0 ${w} ${h}`} role="img" aria-label="Group and personal rolling damage per second">{[0,.5,1].map(mark=><g key={mark}><line x1={l} x2={w-r} y1={y(scale*mark)} y2={y(scale*mark)}/><text x={l-7} y={y(scale*mark)+3} textAnchor="end">{num(scale*mark)}</text></g>)}<polyline className="group" points={line("group")}/><polyline className="me" points={line("me")}/><circle className="group peak" cx={x(groupPeakIndex)} cy={y(groupPeak)} r="4"><title>Group peak {groupPeak.toFixed(1)} DPS</title></circle><circle className="me peak" cx={x(myPeakIndex)} cy={y(myPeak)} r="4"><title>Personal peak {myPeak.toFixed(1)} DPS</title></circle><text x={l} y={h-7}>{series[0]?.second||0}s</text><text x={w-r} y={h-7} textAnchor="end">{series.at(-1)?.second||0}s</text></svg><div className="damage-momentum"><span className={groupMomentum}>Group {groupMomentum}</span><span className={myMomentum}>Me {myMomentum}</span></div></div>:<div className="damage-live-chart-empty">Loading combat timeline...</div>}
  </article>
  <article className="damage-burst-chart"><header><div><span className="eyebrow">Raw output</span><h3>Damage bursts</h3></div><div className="damage-live-legend"><span className="group">Group <b>{num(latestBurst.group)}</b></span><span className="me">Me <b>{num(latestBurst.me)}</b></span></div></header>{events.length?<><div className="damage-burst-plot"><div className="damage-burst-axis"><span>{num(burstPeak)}</span><span>{num(burstPeak/2)}</span><span>0</span></div><div className="damage-burst-bars">{bursts.map(point=><span key={point.second} title={`+${point.second}s - group ${num(point.group)}, me ${num(point.me)}`}><i style={{height:(point.group/burstPeak*100)+"%"}}/><em style={{height:(point.me/burstPeak*100)+"%"}}/></span>)}</div></div><footer><span>{bursts[0]?.second||0}s</span><strong>Damage per second</strong><span>{bursts.at(-1)?.second||0}s</span></footer></>:<div className="damage-live-chart-empty">Loading damage bursts...</div>}</article>
 </section>;
}

function participantDps(player:DamageParticipant){return player.totalDamage/Math.max(1,(time(player.lastDamageAt)-time(player.firstDamageAt))/1000)}

function PlayerCharts({players}:{players:DamageParticipant[]}){
 const damage=[...(players||[])].sort((a,b)=>b.totalDamage-a.totalDamage||a.name.localeCompare(b.name)),rates=[...(players||[])].sort((a,b)=>participantDps(b)-participantDps(a)||b.totalDamage-a.totalDamage),damageMax=Math.max(1,...damage.map(player=>player.totalDamage)),dpsMax=Math.max(1,...rates.map(participantDps));
 const chart=(title:string,caption:string,rows:DamageParticipant[],value:(player:DamageParticipant)=>number,max:number,format:(value:number)=>string)=><article className="damage-player-chart"><header><div><span className="eyebrow">Player ranking</span><h3>{title}</h3><p>{caption}</p></div><strong>{rows.length} player{rows.length===1?"":"s"}</strong></header>{rows.length?<div className="damage-player-bars">{rows.map((player,index)=>{const amount=value(player);return <div key={player.name}><b>{index+1}</b><span title={player.name}>{player.name}<small>{player.hitCount} hit{player.hitCount===1?"":"s"}</small></span><i><em style={{width:(amount/max*100)+"%"}}/></i><strong>{format(amount)}</strong></div>})}</div>:<div className="damage-player-empty">Waiting for player damage…</div>}</article>;
 const total=damage.reduce((sum,player)=>sum+player.totalDamage,0);
 return <section className="damage-player-charts">{chart("Total damage & contribution","Highest contribution first.",damage,player=>player.totalDamage,damageMax,value=>`${num(value)} - ${Math.round(value/Math.max(1,total)*100)}%`)}{chart("DPS","Highest active damage rate first.",rates,participantDps,dpsMax,value=>value.toFixed(1))}</section>
}

function IncomingDamageChart({row}:{row:DamageEncounter}){
 const targets=[...(row.damageTargets||[])].sort((a,b)=>b.totalDamage-a.totalDamage||a.name.localeCompare(b.name)),max=Math.max(1,...targets.map(target=>target.totalDamage));
 return <section className="damage-incoming-chart"><article className="damage-player-chart"><header><div><span className="eyebrow">Incoming damage</span><h3>Mob damage against players</h3><p>Damage dealt by {row.mobName}, highest-hit targets first.</p></div><strong>{num(row.incomingDamage||0)} total</strong></header>{targets.length?<div className="damage-player-bars">{targets.map((target,index)=><div key={target.name}><b>{index+1}</b><span title={target.name}>{target.name}<small>{target.hitCount} hit{target.hitCount===1?"":"s"} - max {num(target.maxHit)}</small></span><i><em style={{width:(target.totalDamage/max*100)+"%"}}/></i><strong>{num(target.totalDamage)}</strong></div>)}</div>:<div className="damage-player-empty">No incoming mob damage captured for this encounter.</div>}</article></section>;
}

function Mix({row}:{row:DamageEncounter}){const pct=row.totalDamage?row.meleeDamage/row.totalDamage*100:0;return <div className="damage-mix" title={`Melee ${num(row.meleeDamage)} - Spell ${num(row.spellDamage)}`}><i style={{width:`${pct}%`}}/><span>{Math.round(pct)}% / {Math.round(100-pct)}%</span></div>}
function Composition({rows}:{rows:DamageEncounter[]}){const melee=rows.reduce((n,r)=>n+r.meleeDamage,0),spell=rows.reduce((n,r)=>n+r.spellDamage,0),total=melee+spell,pct=total?melee/total*100:0;return <div className="damage-composition"><div className="damage-donut" style={{background:total?`conic-gradient(var(--accent) 0 ${pct}%,var(--accent2) ${pct}% 100%)`:"var(--panel2)"}}><span><strong>{num(total)}</strong><small>damage</small></span></div><div className="damage-legend"><div><i className="melee"/><span>Melee</span><strong>{num(melee)}</strong><small>{Math.round(pct)}%</small></div><div><i className="spell"/><span>Spell</span><strong>{num(spell)}</strong><small>{Math.round(100-pct)}%</small></div></div></div>}

function Details({row,close}:{row:DamageEncounterDetail;close:()=>void}){
 const mine=own(row),myDamage=mine?.totalDamage||0,myDps=myDamage/Math.max(1,seconds(row)),myShare=myDamage/Math.max(1,row.totalDamage)*100;
 const playerNames=useMemo(()=>[...new Set(row.events.map(event=>event.attacker))].sort((a,b)=>a.localeCompare(b)),[row.events]);
 const[visiblePlayers,setVisiblePlayers]=useState<Set<string>>(()=>new Set(playerNames));
 useEffect(()=>setVisiblePlayers(new Set(playerNames)),[row.id]);
 const filteredEvents=useMemo(()=>row.events.filter(event=>visiblePlayers.has(event.attacker)),[row.events,visiblePlayers]);
 const togglePlayer=(name:string)=>setVisiblePlayers(current=>{const next=new Set(current);if(next.has(name))next.delete(name);else next.add(name);return next});
 const attacks=useMemo(()=>{const m=new Map<string,{attacker:string,attack:string,type:string,damage:number,hits:number,max:number}>();row.events.forEach(e=>{const k=e.attacker+":"+e.damageType+":"+e.attack,v=m.get(k)||{attacker:e.attacker,attack:e.attack,type:e.damageType,damage:0,hits:0,max:0};v.damage+=e.damage;v.hits++;v.max=Math.max(v.max,e.damage);m.set(k,v)});return [...m.values()]},[row.events]);
 const ac:Column<(typeof attacks)[number]>[]=[{key:"attacker",label:"Player",value:r=>r.attacker},{key:"attack",label:"Attack / Spell",value:r=>r.attack},{key:"type",label:"Type",value:r=>r.type},{key:"damage",label:"Damage",value:r=>r.damage,render:r=>num(r.damage)},{key:"share",label:"Share",value:r=>r.damage/row.totalDamage,render:r=>`${Math.round(r.damage/Math.max(1,row.totalDamage)*100)}%`},{key:"hits",label:"Events",value:r=>r.hits},{key:"average",label:"Average",value:r=>r.damage/r.hits,render:r=>(r.damage/r.hits).toFixed(1)},{key:"max",label:"Max Hit",value:r=>r.max}];
 const eventWeapons=(e:DamageEvent)=>[e.primaryWeapon,e.secondaryWeapon].filter(Boolean).join(" / ")||"Unknown";
 const ec:Column<DamageEvent>[]=[{key:"time",label:"Time",value:e=>e.happenedAt,render:e=>when(e.happenedAt)},{key:"elapsed",label:"Elapsed",value:e=>(time(e.happenedAt)-time(row.startedAt))/1000,render:e=>"+"+Math.max(0,Math.round((time(e.happenedAt)-time(row.startedAt))/1000))+"s"},{key:"attacker",label:"Player",value:e=>e.attacker},{key:"type",label:"Type",value:e=>e.damageType},{key:"attack",label:"Attack / Spell",value:e=>e.attack},{key:"weapons",label:"Equipped Weapons",value:eventWeapons,render:e=><span className="damage-weapons">{eventWeapons(e)}</span>},{key:"damage",label:"Damage",value:e=>e.damage,render:e=><strong className="damage-total">{num(e.damage)}</strong>}];
 const loadouts=[...new Map(row.events.filter(e=>e.primaryWeapon||e.secondaryWeapon).map(e=>{const key=`${e.primaryItemId||0}:${e.primaryWeapon||""}|${e.secondaryItemId||0}:${e.secondaryWeapon||""}`;return[key,{primary:e.primaryWeapon,primaryId:e.primaryItemId,secondary:e.secondaryWeapon,secondaryId:e.secondaryItemId}]})).values()];
 return <Modal title={`${row.character} vs. ${row.mobName}`} onClose={close} footer={<button onClick={close}>Close</button>}><section className="damage-detail-summary"><div className="mine"><span>My damage</span><strong>{num(myDamage)}</strong></div><div className="mine"><span>My DPS</span><strong>{myDps.toFixed(1)}</strong></div><div className="mine"><span>My contribution</span><strong>{Math.round(myShare)}%</strong></div><div className="mine"><span>My events</span><strong>{mine?.hitCount||0}</strong></div><div><span>Group damage</span><strong>{num(row.totalDamage)}</strong></div><div><span>Group DPS</span><strong>{dps(row).toFixed(1)}</strong></div><div><span>Duration</span><strong>{elapsed(seconds(row))}</strong></div><div><span>Group max hit</span><strong>{num(row.maxHit)}</strong></div><div><span>Outcome</span><strong>{result(row.outcome)}</strong></div><div><span>Source</span><strong title={row.sourceFile}>{filename(row.sourceFile)}</strong></div></section><section className="damage-weapon-card"><header><div><span className="eyebrow">Equipment history</span><h3>Weapons used</h3></div><strong>{loadouts.length} loadout{loadouts.length===1?"":"s"}</strong></header><div className="damage-loadouts">{loadouts.length?loadouts.map((loadout,index)=><article key={`${loadout.primaryId||0}-${loadout.secondaryId||0}-${index}`}><b>{index+1}</b><div><span>Primary</span><strong>{loadout.primary||"Empty / unknown"}</strong>{loadout.primaryId&&<small>Item ID {loadout.primaryId}</small>}</div><div><span>Secondary</span><strong>{loadout.secondary||"Empty / unknown"}</strong>{loadout.secondaryId&&<small>Item ID {loadout.secondaryId}</small>}</div></article>):<p>No weapon snapshot was available for this encounter.</p>}</div></section><PlayerCharts players={row.players}/><IncomingDamageChart row={row}/><section className="damage-timeline-panel"><header><div><span className="eyebrow">Damage over time</span><h3>Cumulative damage</h3></div><strong>{filteredEvents.length} of {row.events.length} events</strong></header><div className="damage-timeline-filter"><div><button onClick={()=>setVisiblePlayers(new Set(playerNames))}>Show all</button><button className="primary" onClick={()=>setVisiblePlayers(new Set([row.character]))}>Only me</button><button onClick={()=>setVisiblePlayers(new Set())}>Clear</button></div><div>{playerNames.map(name=><label key={name} className={visiblePlayers.has(name)?"active":""}><input type="checkbox" checked={visiblePlayers.has(name)} onChange={()=>togglePlayer(name)}/><span>{name}</span></label>)}</div></div><Timeline row={row} events={filteredEvents}/></section><section className="damage-detail-grid"><div><h3>Attack breakdown</h3><DataTable rows={attacks} columns={ac} rowKey={r=>r.attacker+":"+r.type+":"+r.attack}/></div><div><h3>Event timeline</h3><DataTable rows={row.events} columns={ec} rowKey={r=>r.id}/></div></section></Modal>;
}

function Timeline({row,events:sourceEvents}:{row:DamageEncounterDetail;events:DamageEvent[]}){const events=[...sourceEvents].sort((a,b)=>time(a.happenedAt)-time(b.happenedAt)||a.id-b.id);if(!events.length)return <div className="damage-empty">Select at least one player to display cumulative damage.</div>;const w=900,h=250,l=55,r=18,t=18,b=35,pw=w-l-r,ph=h-t-b,d=Math.max(1,seconds(row)),max=Math.max(1,events.reduce((total,event)=>total+event.damage,0));let sum=0;const pos=events.map(e=>{sum+=e.damage;const sec=Math.max(0,(time(e.happenedAt)-time(row.startedAt))/1000);return{e,sec,x:l+Math.min(d,sec)/d*pw,y:t+ph-sum/max*ph,sum}}),points=[`${l},${t+ph}`,...pos.map(p=>`${p.x},${p.y}`)].join(" ");return <div className="damage-chart-scroll"><svg className="damage-timeline" viewBox={`0 0 ${w} ${h}`}>{[0,.25,.5,.75,1].map(v=>{const y=t+ph*(1-v);return <g key={v}><line x1={l} x2={w-r} y1={y} y2={y}/><text x={l-8} y={y+3} textAnchor="end">{num(max*v)}</text></g>})}<polygon points={`${points} ${l+pw},${t+ph}`}/><polyline points={points}/>{pos.map(p=><circle key={p.e.id} cx={p.x} cy={p.y} r="3"><title>+{Math.round(p.sec)}s - {p.e.attack}: {num(p.e.damage)} - {num(p.sum)} cumulative</title></circle>)}<text x={l} y={h-10}>0s</text><text x={l+pw} y={h-10} textAnchor="end">{elapsed(d)}</text></svg></div>}
