import {useCallback,useEffect,useState} from "react";
import {open as openDialog} from "@tauri-apps/plugin-dialog";
import {importReplayFile,listReplayFiles,loadReplayFile} from "../../shared/backend";
import type {ReplayFile,ReplayFileEntry} from "../../shared/contracts";
import {DataTable,IconButton,when,type Column} from "../ui";
import {formatCombatClock} from "./meterModel";

const number=(value:number)=>Math.round(value).toLocaleString();

export function ReplayLibrary({onLoad}:{onLoad:(replay:ReplayFile,path:string)=>Promise<void>}){
 const[rows,setRows]=useState<ReplayFileEntry[]>([]);
 const[loading,setLoading]=useState(true);
 const[working,setWorking]=useState("");
 const[error,setError]=useState("");
 const refresh=useCallback(async()=>{setLoading(true);setError("");try{setRows(await listReplayFiles())}catch(reason){setError(String(reason))}finally{setLoading(false)}},[]);
 useEffect(()=>{void refresh()},[refresh]);
 const load=async(row:ReplayFileEntry)=>{setWorking(row.path);setError("");try{await onLoad(await loadReplayFile(row.path),row.path)}catch(reason){setError(String(reason))}finally{setWorking("")}};
 const importFiles=async()=>{setError("");try{const selected=await openDialog({multiple:true,directory:false,filters:[{name:"EverQuest fight replay",extensions:["json"]}]});const paths=typeof selected==="string"?[selected]:selected||[];if(!paths.length)return;setWorking("import");for(const path of paths)await importReplayFile(path);await refresh()}catch(reason){setError(String(reason))}finally{setWorking("")}};
 const columns:Column<ReplayFileEntry>[]=[
  {key:"title",label:"Fight",value:row=>row.title,render:row=><div className="replay-title"><strong>{row.mobName}</strong><small>{row.activeCharacter}</small></div>},
  {key:"file",label:"File",value:row=>row.fileName,render:row=><span className="replay-file" title={row.path}>{row.fileName}</span>},
  {key:"started",label:"Engaged",value:row=>row.startedAt,render:row=>when(row.startedAt)},
  {key:"duration",label:"Duration",value:row=>row.durationSeconds,render:row=>formatCombatClock(row.durationSeconds)},
  {key:"damage",label:"Damage",value:row=>row.totalDamage,render:row=><strong>{number(row.totalDamage)}</strong>},
  {key:"fighters",label:"Fighters",value:row=>row.participantCount},
  {key:"events",label:"Lines / Events",value:row=>row.eventCount,render:row=>String(row.lineCount)+" / "+String(row.eventCount)},
  {key:"procs",label:"Procs",value:row=>row.procCount},
  {key:"dot",label:"DoT Damage",value:row=>row.dotDamage,render:row=>number(row.dotDamage)},
  {key:"reviews",label:"Reviews",value:row=>row.coachReviewCount},
  {key:"quality",label:"Capture",value:row=>row.captureMode,render:row=><span className={"replay-quality "+row.captureMode}>{row.captureMode==="source-window"?"Full context":"Reduced context"}</span>},
  {key:"saved",label:"Saved",value:row=>row.savedAt,render:row=>when(row.savedAt)},
 ];
 return <section className="replay-library card">
  <header><div><span className="eyebrow">External training files</span><h2>Saved fight library</h2><p>Recorded fights are portable <code>.eqfight.json</code> files. Loading one re-runs the current production parser.</p></div><div className="button-row"><button disabled={working==="import"} onClick={importFiles}>{working==="import"?"Importing...":"Import replay files"}</button><IconButton icon="refresh" label="Refresh replay library" disabled={loading} onClick={refresh}/></div></header>
  {error&&<div className="alert"><span>{error}</span><button onClick={()=>setError("")}>x</button></div>}
  {loading?<div className="replay-library-loading">Loading saved fights...</div>:<DataTable rows={rows} columns={columns} rowKey={row=>row.path} empty="No fights saved yet. Use the save icon on a recorded Damage Tracker encounter." actions={row=><IconButton icon="play" label={"Load "+row.title+" into training lab"} disabled={!!working} onClick={()=>load(row)}/>}/>}
 </section>;
}
