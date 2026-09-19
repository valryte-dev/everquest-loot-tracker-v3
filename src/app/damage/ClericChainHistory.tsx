import {useCallback,useEffect,useMemo,useRef,useState} from "react";
import type {ClericHealReplayEntry,ClericHealReplayFile} from "../../shared/contracts";
import {listClericHealReplayFiles,loadClericHealReplayFile,saveClericHealReplay} from "../../shared/backend";
import {DataTable,IconButton,Modal,type Column,when} from "../ui";
import {encounterFromReplay,replayCalls,type ClericChainEncounter} from "./chReplay";
import {formatCombatClock} from "./meterModel";
import {openEncounterInChLab} from "./chLabNavigation";

const callTime=(value:string)=>{const parsed=Date.parse(value.includes("T")?value:value.replace(" ","T"));return Number.isFinite(parsed)?parsed:0};

export function ClericChainHistory({encounters}:{encounters:ClericChainEncounter[]}){
 const[detail,setDetail]=useState<ClericChainEncounter|null>(null),[replay,setReplay]=useState<ClericChainEncounter|null>(null),[saved,setSaved]=useState<ClericHealReplayEntry[]>([]),[loading,setLoading]=useState(true),[working,setWorking]=useState(""),[notice,setNotice]=useState(""),[error,setError]=useState("");
 const refresh=useCallback(async()=>{setLoading(true);try{setSaved(await listClericHealReplayFiles())}catch(reason){setError(String(reason))}finally{setLoading(false)}},[]);
 useEffect(()=>{void refresh();const listener=()=>void refresh();window.addEventListener("ch-replay-saved",listener);return()=>window.removeEventListener("ch-replay-saved",listener)},[refresh]);
 const save=async(row:ClericChainEncounter)=>{setWorking(row.id);setError("");try{await saveClericHealReplay(row.character,row.targetMob,replayCalls(row));setNotice(`Saved ${row.targetMob} CH encounter for replay.`);await refresh()}catch(reason){setError(String(reason))}finally{setWorking("")}};
 const load=async(row:ClericHealReplayEntry)=>{setWorking(row.path);setError("");try{const file:ClericHealReplayFile=await loadClericHealReplayFile(row.path);setReplay(encounterFromReplay(file,row.path))}catch(reason){setError(String(reason))}finally{setWorking("")}};
 const columns:Column<ClericChainEncounter>[]=[
  {key:"time",label:"Started",value:row=>row.startedAt,render:row=>when(row.startedAt)},
  {key:"mob",label:"Tank",value:row=>row.targetMob,render:row=><button className="ch-encounter-link" onClick={()=>setDetail(row)}>{row.targetMob}</button>},
  {key:"character",label:"Log Toon",value:row=>row.character},
  {key:"calls",label:"Calls",value:row=>row.callCount},
  {key:"healers",label:"Healers",value:row=>row.healerCount},
  {key:"status",label:"Status",value:row=>row.status,render:row=><span className={`pill ${row.status==="concluded"?"success":"info"}`}>{row.status==="concluded"?"Concluded":"Active"}</span>},
  {key:"duration",label:"Duration",value:row=>row.durationSeconds,render:row=>formatCombatClock(row.durationSeconds)},
  {key:"average",label:"Average Gap",value:row=>row.averageGapSeconds,render:row=>row.averageGapSeconds.toFixed(1)+"s"},
  {key:"longest",label:"Longest Gap",value:row=>row.longestGapSeconds,render:row=>row.longestGapSeconds.toFixed(1)+"s"},
 ];
 const savedColumns:Column<ClericHealReplayEntry>[]=[
  {key:"time",label:"Saved",value:row=>row.savedAt,render:row=>when(row.savedAt)},
  {key:"mob",label:"Tank",value:row=>row.targetMob},
  {key:"character",label:"Log Toon",value:row=>row.character},
  {key:"calls",label:"Calls",value:row=>row.callCount},
  {key:"healers",label:"Healers",value:row=>row.healerCount},
  {key:"duration",label:"Duration",value:row=>row.durationSeconds,render:row=>formatCombatClock(row.durationSeconds)},
  {key:"average",label:"Average Gap",value:row=>row.averageGapSeconds,render:row=>row.averageGapSeconds.toFixed(1)+"s"},
  {key:"file",label:"Replay File",value:row=>row.fileName,render:row=><span className="replay-file" title={row.path}>{row.fileName}</span>},
 ];
 return <>
  <article className="card cleric-chain-history"><header><div><h2>CH encounter history</h2><p>Each row is one chain encounter. Open it to inspect every call or send it to the CH Lab.</p></div><small>{encounters.length.toLocaleString()} encounters from the newest call window</small></header><DataTable rows={encounters} columns={columns} rowKey={row=>row.id} empty="No Complete Heal encounters captured yet." actions={row=><><IconButton icon="external" label={`Open ${row.targetMob} in CH Lab`} onClick={()=>openEncounterInChLab(row)}/><IconButton icon="save" label={`Save ${row.targetMob} CH encounter`} disabled={!!working} onClick={()=>void save(row)}/><IconButton icon="play" label={`Quick replay ${row.targetMob} CH encounter`} disabled={!!working} onClick={()=>setReplay(row)}/><IconButton icon="external" label={`View ${row.targetMob} CH calls`} onClick={()=>setDetail(row)}/></>}/></article>
  <article className="card ch-replay-library"><header><div><span className="eyebrow">External replay files</span><h2>Saved CH encounter library</h2><p>Portable <code>.eqch.json</code> files can be replayed here or opened in the CH Lab.</p></div><IconButton icon="refresh" label="Refresh saved CH encounters" disabled={loading} onClick={()=>void refresh()}/></header>{loading?<div className="replay-library-loading">Loading saved CH encounters...</div>:<DataTable rows={saved} columns={savedColumns} rowKey={row=>row.path} empty="No CH encounters saved yet." actions={row=><><IconButton icon="external" label={`Open saved ${row.targetMob} in CH Lab`} disabled={!!working} onClick={async()=>{setWorking(row.path);setError("");try{const file=await loadClericHealReplayFile(row.path);openEncounterInChLab(encounterFromReplay(file,row.path))}catch(reason){setError(String(reason));setWorking("")}}}/><IconButton icon="play" label={`Quick replay saved ${row.targetMob} encounter`} disabled={!!working} onClick={()=>void load(row)}/></>} />}</article>
  {notice&&<div className="alert success"><span>{notice}</span><button onClick={()=>setNotice("")}>x</button></div>}{error&&<div className="alert"><span>{error}</span><button onClick={()=>setError("")}>x</button></div>}
  {detail&&<ClericChainDetail encounter={detail} close={()=>setDetail(null)} replay={()=>{setDetail(null);setReplay(detail)}}/>}
  {replay&&<ClericChainReplay encounter={replay} close={()=>setReplay(null)}/>} 
 </>;
}

function CallTable({encounter,rows=encounter.calls}:{encounter:ClericChainEncounter;rows?:ClericChainEncounter["calls"]}){
 const columns:Column<ClericChainEncounter["calls"][number]>[]=[
  {key:"time",label:"Called",value:row=>row.happenedAt,render:row=>when(row.happenedAt)},
  {key:"cleric",label:"Cleric",value:row=>row.clericName,render:row=><strong>{row.clericName}</strong>},
  {key:"number",label:"Call",value:row=>row.callNumber,render:row=>"#"+String(row.callNumber).padStart(3,"0")},
  {key:"target",label:"Heal Target",value:row=>row.targetName||"",render:row=>row.targetName||"-"},
  {key:"gap",label:"Chain Gap",value:row=>row.gapSeconds??-1,render:row=>row.gapSeconds===undefined?"Start":row.gapSeconds.toFixed(1)+"s"},
  {key:"rotation",label:"Cleric Rotation",value:row=>row.clericGapSeconds??-1,render:row=>row.clericGapSeconds===undefined?"-":row.clericGapSeconds.toFixed(1)+"s"},
 ];
 return <DataTable rows={rows} columns={columns} rowKey={row=>row.id} empty="No calls at this replay position."/>;
}

function EncounterStats({encounter}:{encounter:ClericChainEncounter}){return <div className="ch-replay-stats"><div><span>Tank</span><strong>{encounter.targetMob}</strong></div><div><span>Calls</span><strong>{encounter.callCount}</strong></div><div><span>Healers</span><strong>{encounter.healerCount}</strong></div><div><span>Duration</span><strong>{formatCombatClock(encounter.durationSeconds)}</strong></div><div><span>Average gap</span><strong>{encounter.averageGapSeconds.toFixed(1)}s</strong></div><div><span>Longest gap</span><strong>{encounter.longestGapSeconds.toFixed(1)}s</strong></div></div>}

function ClericConsistency({encounter}:{encounter:ClericChainEncounter}){
 if(encounter.status!=="concluded")return <section className="ch-consistency pending"><strong>Per-cleric consistency pending</strong><span>Standard deviation is finalized after 15 seconds without a CH call or when the encounter ends.</span></section>;
 const columns:Column<ClericChainEncounter["clericStats"][number]>[]=[
  {key:"cleric",label:"Cleric",value:row=>row.name,render:row=><strong>{row.name}</strong>},
  {key:"calls",label:"Calls",value:row=>row.callCount},
  {key:"intervals",label:"Rotation Samples",value:row=>row.intervalCount},
  {key:"average",label:"Average Rotation",value:row=>row.averageRotationSeconds,render:row=>row.intervalCount?row.averageRotationSeconds.toFixed(1)+"s":"-"},
  {key:"deviation",label:"Std. Deviation",value:row=>row.standardDeviationSeconds,render:row=>row.intervalCount?row.standardDeviationSeconds.toFixed(1)+"s":"-"},
  {key:"range",label:"Rotation Range",value:row=>row.longestRotationSeconds-row.shortestRotationSeconds,render:row=>row.intervalCount?`${row.shortestRotationSeconds.toFixed(1)}s - ${row.longestRotationSeconds.toFixed(1)}s`:"-"},
 ];
 return <section className="ch-consistency"><header><div><span className="eyebrow">Final rotation analysis</span><h3>Per-cleric consistency</h3><p>Population standard deviation measures variation between each cleric's own consecutive calls. Lower is more consistent.</p></div><strong>{encounter.clericStats.length} clerics</strong></header><DataTable rows={encounter.clericStats} columns={columns} rowKey={row=>row.name} empty="No cleric rotation samples were available."/></section>;
}
function ClericChainDetail({encounter,close,replay}:{encounter:ClericChainEncounter;close:()=>void;replay:()=>void}){return <Modal title={`CH encounter - ${encounter.targetMob}`} onClose={close} footer={<><button onClick={close}>Close</button><button className="primary" onClick={replay}>Replay encounter</button></>}><EncounterStats encounter={encounter}/><ClericConsistency encounter={encounter}/><CallTable encounter={encounter}/></Modal>}

function ClericChainReplay({encounter,close}:{encounter:ClericChainEncounter;close:()=>void}){
 const[cursor,setCursor]=useState(0),[playing,setPlaying]=useState(false),[speed,setSpeed]=useState(1);
 const timer=useRef<number|undefined>(undefined),calls=encounter.calls,start=callTime(calls[0]?.happenedAt||encounter.startedAt),end=callTime(calls.at(-1)?.happenedAt||encounter.endedAt),duration=Math.max(1,end-start),position=cursor?Math.max(0,callTime(calls[Math.min(cursor-1,calls.length-1)].happenedAt)-start):0;
 useEffect(()=>{window.clearTimeout(timer.current);if(!playing||cursor>=calls.length)return;const previous=cursor?callTime(calls[cursor-1].happenedAt):start,next=callTime(calls[cursor].happenedAt),delay=Math.max(180,Math.min(4000,(next-previous)/speed));timer.current=window.setTimeout(()=>setCursor(value=>value+1),delay);return()=>window.clearTimeout(timer.current)},[playing,cursor,calls,start,speed]);
 useEffect(()=>{if(cursor>=calls.length)setPlaying(false)},[cursor,calls.length]);
 const seek=(value:number)=>{setPlaying(false);const absolute=start+value;setCursor(calls.filter(call=>callTime(call.happenedAt)<=absolute).length)};
 return <Modal title={`Replay CH encounter - ${encounter.targetMob}`} onClose={close} footer={<button onClick={close}>Close</button>}><EncounterStats encounter={encounter}/><ClericConsistency encounter={encounter}/><div className="ch-replay-controls"><IconButton icon={playing?"pause":"play"} label={playing?"Pause CH replay":"Play CH replay"} disabled={!calls.length} onClick={()=>{if(cursor>=calls.length)setCursor(0);setPlaying(value=>!value)}}/><button onClick={()=>{setPlaying(false);setCursor(0)}}>Reset</button><button onClick={()=>{setPlaying(false);setCursor(value=>Math.max(0,value-1))}}>Previous call</button><button onClick={()=>{setPlaying(false);setCursor(value=>Math.min(calls.length,value+1))}}>Next call</button><label><span>Speed</span><select value={speed} onChange={event=>setSpeed(Number(event.target.value))}><option value={0.5}>0.5x</option><option value={1}>1x</option><option value={2}>2x</option><option value={4}>4x</option></select></label><strong>{cursor} / {calls.length} calls</strong></div><label className="ch-replay-scrubber"><input type="range" min="0" max={duration} value={position} onChange={event=>seek(Number(event.target.value))}/><span>{formatCombatClock(position/1000)} / {formatCombatClock(duration/1000)}</span></label><div className="cleric-chain-flow ch-replay-flow">{calls.slice(0,cursor).map((call,index)=><article key={call.id}><div><b>#{String(call.callNumber).padStart(3,"0")}</b><span>{call.clericName}</span></div><strong>{call.gapSeconds===undefined?"Start":"+"+call.gapSeconds.toFixed(1)+"s"}</strong><small>{call.targetName?"on "+call.targetName:call.channel}</small>{index<cursor-1&&<i/>}</article>)}</div><CallTable encounter={encounter} rows={calls.slice(0,cursor)}/></Modal>;
}
