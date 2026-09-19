import {useCallback,useEffect,useMemo,useState} from "react";
import {getChTrainingPreview,listClericHealReplayFiles,loadClericHealReplayFile} from "../../shared/backend";
import type {ChTrainingLine,ChTrainingReport,ClericHealReplayEntry} from "../../shared/contracts";
import {DataTable,IconButton,type Column,when} from "../ui";
import {ClericChainPulse} from "./ClericChainPulse";
import {buildClericChainTimeline} from "./model";
import {encounterFromReplay,type ClericChainEncounter} from "./chReplay";
import {takePendingChLabEncounter} from "./chLabNavigation";
import {formatCombatClock} from "./meterModel";

const SAMPLE=`[Mon Sep 07 20:00:00 2026] Bakamore tells the guild, 'LoF 001 CH - MainTank'
[Mon Sep 07 20:00:09 2026] Clerica tells the guild, 'LoF 002 CH - MainTank'
[Mon Sep 07 20:00:19 2026] Lightkeeper tells the guild, 'lof 003 ch - MainTank'
[Mon Sep 07 20:00:28 2026] You say to your guild, 'LoF 004 CH - MainTank'
[Mon Sep 07 20:00:40 2026] Bakamore tells the guild, 'LoF 005 CH - MainTank'
[Mon Sep 07 20:00:49 2026] Clerica tells the guild, 'LoF 006 CH - MainTank'
[Mon Sep 07 20:00:59 2026] Lightkeeper tells the guild, 'LoF 007 CH - MainTank'
[Mon Sep 07 20:01:10 2026] You say to your guild, 'LoF 008 CH - MainTank'`;

const callTime=(value:string)=>{const parsed=Date.parse(value.includes("T")?value:value.replace(" ","T"));return Number.isFinite(parsed)?parsed:0};

export function ChTrainingPage({activeCharacter}:{activeCharacter?:string}){
 const[text,setText]=useState(""),[character,setCharacter]=useState(activeCharacter||"Tester"),[report,setReport]=useState<ChTrainingReport|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState(""),[openGap,setOpenGap]=useState(0),[clockRunning,setClockRunning]=useState(false);
 const[replay,setReplay]=useState<ClericChainEncounter|null>(()=>takePendingChLabEncounter()),[elapsed,setElapsed]=useState(0),[replayPlaying,setReplayPlaying]=useState(false),[speed,setSpeed]=useState(1);
 const[saved,setSaved]=useState<ClericHealReplayEntry[]>([]),[libraryLoading,setLibraryLoading]=useState(true),[loadingPath,setLoadingPath]=useState("");
 const refreshLibrary=useCallback(async()=>{setLibraryLoading(true);try{setSaved(await listClericHealReplayFiles())}catch(reason){setError(String(reason))}finally{setLibraryLoading(false)}},[]);
 useEffect(()=>{void refreshLibrary();const listener=()=>void refreshLibrary();window.addEventListener("ch-replay-saved",listener);return()=>window.removeEventListener("ch-replay-saved",listener)},[refreshLibrary]);
 useEffect(()=>{if(!clockRunning||replay)return;const timer=window.setInterval(()=>setOpenGap(value=>Math.min(30,value+.1)),100);return()=>window.clearInterval(timer)},[clockRunning,replay]);
 const replayStart=callTime(replay?.calls[0]?.happenedAt||""),replayEnd=callTime(replay?.calls.at(-1)?.happenedAt||""),duration=Math.max(1000,replayEnd-replayStart);
 useEffect(()=>{if(!replayPlaying||!replay)return;const timer=window.setInterval(()=>setElapsed(value=>{const next=Math.min(duration,value+100*speed);if(next>=duration)setReplayPlaying(false);return next}),100);return()=>window.clearInterval(timer)},[replayPlaying,replay,duration,speed]);
 const analyze=async(source=text)=>{setBusy(true);setError("");setClockRunning(false);setReplayPlaying(false);setReplay(null);try{setReport(await getChTrainingPreview(source,character||"Tester"));setOpenGap(0)}catch(reason){setError(String(reason))}finally{setBusy(false)}};
 const timeline=useMemo(()=>buildClericChainTimeline(report?.calls||[]),[report]),session=timeline.at(-1)?.session,parsedCalls=timeline.filter(call=>call.session===session),gaps=parsedCalls.flatMap(call=>call.gapSeconds===undefined?[]:[call.gapSeconds]);
 const replayOffsets=useMemo(()=>replay?.calls.map(call=>Math.max(0,callTime(call.happenedAt)-replayStart))||[],[replay,replayStart]);
 const replayCalls=replay?.calls.filter((_,index)=>replayOffsets[index]<=elapsed+1)||[],lastReplayCall=replayCalls.at(-1),replayGap=lastReplayCall?Math.max(0,(replayStart+elapsed-callTime(lastReplayCall.happenedAt))/1000):0;
 const columns:Column<ChTrainingLine>[]=[
  {key:"line",label:"Line",value:row=>row.lineNumber},
  {key:"time",label:"Time",value:row=>row.happenedAt||"",render:row=>row.happenedAt?when(row.happenedAt):"-"},
  {key:"status",label:"Status",value:row=>row.status,render:row=><span className={`dot-lab-status ${row.status==="recognized"?"recognized":row.status==="other"?"pending":"ignored"}`}>{row.status}</span>},
  {key:"event",label:"Parser Event",value:row=>row.parserEvent},
  {key:"summary",label:"Interpretation",value:row=>row.summary},
  {key:"raw",label:"Raw Log Line",value:row=>row.rawLine,render:row=><span className="ch-lab-raw" title={row.rawLine}>{row.rawLine}</span>},
 ];
 const savedColumns:Column<ClericHealReplayEntry>[]=[
  {key:"time",label:"Saved",value:row=>row.savedAt,render:row=>when(row.savedAt)},
  {key:"mob",label:"Tank",value:row=>row.targetMob},
  {key:"character",label:"Log Toon",value:row=>row.character},
  {key:"calls",label:"Calls",value:row=>row.callCount},
  {key:"healers",label:"Healers",value:row=>row.healerCount},
  {key:"duration",label:"Duration",value:row=>row.durationSeconds,render:row=>formatCombatClock(row.durationSeconds)},
  {key:"average",label:"Average Gap",value:row=>row.averageGapSeconds,render:row=>row.averageGapSeconds.toFixed(1)+"s"},
 ];
 const activateReplay=(encounter:ClericChainEncounter)=>{setReplay(encounter);setReport(null);setReplayPlaying(false);setElapsed(0);setClockRunning(false);setCharacter(encounter.character);setError("")};
 const loadSaved=async(row:ClericHealReplayEntry)=>{setLoadingPath(row.path);setError("");try{const file=await loadClericHealReplayFile(row.path);activateReplay(encounterFromReplay(file,row.path))}catch(reason){setError(String(reason))}finally{setLoadingPath("")}};
 const loadSample=()=>{setText(SAMPLE);setReport(null);setReplay(null);setOpenGap(0);setClockRunning(false);setReplayPlaying(false)};
 const clear=()=>{setText("");setReport(null);setReplay(null);setError("");setOpenGap(0);setElapsed(0);setClockRunning(false);setReplayPlaying(false)};
 const nextCall=()=>{setReplayPlaying(false);const next=replayOffsets.find(value=>value>elapsed+1);setElapsed(next===undefined?duration:next)};
 const previousCall=()=>{setReplayPlaying(false);const previous=[...replayOffsets].reverse().find(value=>value<elapsed-1);setElapsed(previous??0)};
 return <section className="ch-lab">
  <section className="card ch-lab-editor"><header><div><span className="eyebrow">Safe parser sandbox</span><h2>Complete Heal Chain Lab</h2><p>Paste guild CH calls, load a recorded encounter, and exercise the exact live Chain Pulse visualization.</p></div><div className="button-row"><button onClick={loadSample}>Load sample</button><button disabled={!text&&!report&&!replay} onClick={clear}>Clear</button><button className="primary" disabled={busy||!text.trim()} onClick={()=>void analyze()}>{busy?"Analyzing...":"Analyze CH lines"}</button></div></header><label className="field"><span>Active character represented by "You"</span><input value={character} onChange={event=>setCharacter(event.target.value)} placeholder="Tester"/></label><textarea value={text} onChange={event=>{setText(event.target.value);setReport(null);setReplay(null)}} spellCheck={false} placeholder="Paste complete [timestamp] guild CH log lines here..."/>{error&&<div className="alert"><strong>CH Lab error</strong><span>{error}</span><button onClick={()=>setError("")}>x</button></div>}</section>
  <section className="card ch-lab-library"><header><div><span className="eyebrow">History connection</span><h2>Saved CH encounters</h2><p>Load portable history replays here, or use the new lab action on any CH history row.</p></div><IconButton icon="refresh" label="Refresh saved CH encounter library" disabled={libraryLoading} onClick={()=>void refreshLibrary()}/></header>{libraryLoading?<div className="replay-library-loading">Loading saved CH encounters...</div>:<DataTable rows={saved} columns={savedColumns} rowKey={row=>row.path} empty="No saved CH encounters yet. Save one from CH history or open a history row directly." actions={row=><IconButton icon="play" label={`Load ${row.targetMob} in CH Lab`} disabled={!!loadingPath} onClick={()=>void loadSaved(row)}/>} />}</section>
  {replay&&<section className="card ch-lab-replay"><header><div><span className="eyebrow">Recorded encounter replay</span><h2>{replay.targetMob}</h2><p>{replay.character} / {when(replay.startedAt)}</p></div><strong>{replayCalls.length} / {replay.calls.length} calls</strong></header><section className="ch-lab-summary stats"><article><span>Healers</span><strong>{replay.healerCount}</strong><small>Distinct chain participants</small></article><article><span>Average gap</span><strong>{replay.averageGapSeconds.toFixed(1)}s</strong><small>{replay.longestGapSeconds.toFixed(1)}s longest</small></article><article><span>Duration</span><strong>{formatCombatClock(replay.durationSeconds)}</strong><small>{formatCombatClock(elapsed/1000)} replayed</small></article></section><div className="ch-replay-controls"><IconButton icon={replayPlaying?"pause":"play"} label={replayPlaying?"Pause CH replay":"Play CH replay"} onClick={()=>{if(elapsed>=duration)setElapsed(0);setReplayPlaying(value=>!value)}}/><button onClick={()=>{setReplayPlaying(false);setElapsed(0)}}>Reset</button><button onClick={previousCall}>Previous call</button><button onClick={nextCall}>Next call</button><label><span>Speed</span><select value={speed} onChange={event=>setSpeed(Number(event.target.value))}><option value={0.5}>0.5x</option><option value={1}>1x</option><option value={2}>2x</option><option value={4}>4x</option></select></label><strong>{formatCombatClock(elapsed/1000)} / {formatCombatClock(duration/1000)}</strong></div><label className="ch-replay-scrubber"><input type="range" min="0" max={duration} step="100" value={elapsed} onChange={event=>{setReplayPlaying(false);setElapsed(Number(event.target.value))}}/><span>Replay timeline</span></label>{replayCalls.length?<><ClericChainPulse calls={replayCalls} currentGap={replayGap}/><ChainFlow calls={replayCalls}/></>:<div className="damage-empty">Press play or step forward to begin the encounter.</div>}</section>}
  {report&&<><section className="ch-lab-summary stats"><article><span>Input lines</span><strong>{report.lineCount}</strong><small>Production parser input</small></article><article><span>CH calls found</span><strong>{report.recognizedCount}</strong><small>Guild channel only</small></article><article><span>Current session</span><strong>{parsedCalls.length}</strong><small>{gaps.length?`${(gaps.reduce((sum,gap)=>sum+gap,0)/gaps.length).toFixed(1)}s average gap`:"Learning cadence"}</small></article></section><section className="card ch-lab-preview"><header><div><span className="eyebrow">Live control preview</span><h2>Chain Pulse simulator</h2><p>Move the clock beyond the learned cadence to preview due and overdue states.</p></div><strong>{openGap.toFixed(1)}s since last call</strong></header>{parsedCalls.length?<><ClericChainPulse calls={parsedCalls} currentGap={openGap}/><div className="ch-lab-clock"><button onClick={()=>setOpenGap(value=>Math.max(0,value-1))}>-1s</button><button className={clockRunning?"active":""} onClick={()=>setClockRunning(value=>!value)}>{clockRunning?"Pause clock":"Run clock"}</button><button onClick={()=>setOpenGap(value=>Math.min(30,value+1))}>+1s</button><button onClick={()=>setOpenGap(value=>Math.min(30,value+5))}>+5s</button><button onClick={()=>{setClockRunning(false);setOpenGap(0)}}>Reset</button><label><input type="range" min="0" max="30" step="0.1" value={openGap} onChange={event=>{setClockRunning(false);setOpenGap(Number(event.target.value))}}/><span>{openGap.toFixed(1)}s</span></label></div><ChainFlow calls={parsedCalls}/></>:<div className="damage-empty">No guild CH calls were recognized in the latest session.</div>}</section><section className="card ch-lab-interpretation"><header><div><span className="eyebrow">Production parser decisions</span><h2>Line interpretation</h2><p>Search and sort exactly how each pasted line was classified.</p></div><small>{report.recognizedCount} recognized / {report.ignoredCount} not CH calls</small></header><DataTable rows={report.lines} columns={columns} rowKey={row=>row.lineNumber} empty="No lines were analyzed."/></section></>}
 </section>;
}

function ChainFlow({calls}:{calls:ReturnType<typeof buildClericChainTimeline>}){return <div className="cleric-chain-flow ch-lab-flow">{calls.map((call,index)=><article key={call.id}><div><b>#{String(call.callNumber).padStart(3,"0")}</b><span>{call.clericName}</span></div><strong>{call.gapSeconds===undefined?"Start":"+"+call.gapSeconds.toFixed(1)+"s"}</strong><small>{call.targetName?"on "+call.targetName:call.channel}</small>{index<calls.length-1&&<i/>}</article>)}</div>}
