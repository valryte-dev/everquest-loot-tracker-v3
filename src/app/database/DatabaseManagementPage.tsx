import {useCallback,useEffect,useMemo,useState} from "react";
import {getCombatCleanupPreview,getDatabaseStats} from "../../shared/backend";
import type {DatabaseCleanupPreview,DatabaseStats,DatabaseStorageCategory,LoadingState,Runner} from "../../shared/contracts";
import {DataTable,when,type Column} from "../ui";

const retentionChoices=[30,90,180,365,730];

export function formatBytes(value:number):string {
 if(!Number.isFinite(value)||value<=0)return "0 B";
 const units=["B","KB","MB","GB","TB"];
 const unit=Math.min(Math.floor(Math.log(value)/Math.log(1024)),units.length-1);
 const amount=value/1024**unit;
 return `${amount.toFixed(unit===0?0:amount>=100?0:amount>=10?1:2)} ${units[unit]}`;
}

export function DatabaseManagementPage({run}:{run:Runner}){
 const[state,setState]=useState<LoadingState<DatabaseStats>>({kind:"loading"});
 const[keepDays,setKeepDays]=useState(180);
 const[preview,setPreview]=useState<LoadingState<DatabaseCleanupPreview>|null>(null);
 const[backupMessage,setBackupMessage]=useState("");
 const load=useCallback(()=>{setState({kind:"loading"});getDatabaseStats().then(value=>setState({kind:"ready",value})).catch(error=>setState({kind:"error",message:String(error)}))},[]);
 useEffect(()=>{load()},[load]);
 const previewRetention=()=>{setPreview({kind:"loading"});getCombatCleanupPreview(keepDays).then(value=>setPreview({kind:"ready",value})).catch(error=>setPreview({kind:"error",message:String(error)}))};
 const backup=async()=>{setBackupMessage("Creating a consistent online backup…");const result=await run("database.backup");if(isBackupResult(result))setBackupMessage(`Backup created: ${result.path}`);else setBackupMessage("Backup did not complete. Check Application Logs for details.")};
 if(state.kind==="loading")return <section className="db-loading" role="status"><div className="spinner"/><h2>Analyzing database</h2><p>Counting records and sampling storage without changing any data.</p></section>;
 if(state.kind==="error")return <section className="db-error"><h2>Database analysis failed</h2><p>{state.message}</p><button className="primary" onClick={load}>Try again</button></section>;
 const data=state.value;
 return <div className="database-management">
  <section className="db-overview">
   <article><span>Database file</span><strong>{formatBytes(data.databaseBytes)}</strong><small>{data.databasePath}</small></article>
   <article><span>Free inside database</span><strong>{formatBytes(data.reclaimableBytes)}</strong><small>{data.freelistPages.toLocaleString()} reusable pages</small></article>
   <article><span>Write-ahead log</span><strong>{formatBytes(data.walBytes)}</strong><small>{data.journalMode.toUpperCase()} journal mode</small></article>
   <article><span>Health</span><strong className={data.integrity.toLowerCase()==="ok"?"healthy":"unhealthy"}>{data.integrity}</strong><small>Schema {data.schemaVersion} · checked {when(data.generatedAt)}</small></article>
  </section>

  <section className="db-callout">
   <div><span className="eyebrow">Largest growth source</span><h2>Combat hit detail</h2><p>Individual outgoing and incoming hits dominate this database. Encounter totals, player summaries, DPS charts, and target summaries are stored separately, so an approved retention policy can remove old hit-by-hit detail while preserving those summaries.</p></div>
   <div className="db-callout-actions"><button onClick={load}>Refresh analysis</button><button className="primary" onClick={backup}>Create backup</button></div>
  </section>
  {backupMessage&&<p className="db-status" role="status">{backupMessage}</p>}

  <StorageBreakdown rows={data.categories} databaseBytes={data.databaseBytes}/>

  <section className="db-retention">
   <header><div><span className="eyebrow">Preview only</span><h2>Combat detail retention</h2><p>Choose how much hit-by-hit detail you might keep. This calculates impact and does not delete or modify anything.</p></div><span className="pill warn">No deletion enabled</span></header>
   <div className="db-retention-controls"><label><span>Keep detailed hits for</span><select value={keepDays} onChange={event=>{setKeepDays(Number(event.target.value));setPreview(null)}}>{retentionChoices.map(days=><option key={days} value={days}>{days} days</option>)}</select></label><button className="primary" onClick={previewRetention}>Preview impact</button></div>
   {preview?.kind==="loading"&&<p className="db-preview-loading" role="status">Calculating eligible rows…</p>}
   {preview?.kind==="error"&&<p className="db-preview-error">{preview.message}</p>}
   {preview?.kind==="ready"&&<PreviewResult value={preview.value}/>} 
  </section>

  <section className="db-guidance">
   <article><h3>What cleanup would preserve</h3><ul><li>Encounter totals and outcomes</li><li>Per-player total damage and hit counts</li><li>Incoming damage summaries</li><li>DPS and historical analytics</li></ul></article>
   <article><h3>What old-detail cleanup would remove</h3><ul><li>Individual hit rows before the cutoff</li><li>Old encounter event timelines</li><li>Per-hit raw diagnostic text</li><li>Per-hit weapon references for that old detail</li></ul></article>
   <article><h3>Why the file will not shrink immediately</h3><p>Deletion creates reusable pages inside SQLite. A separate optimize-and-shrink operation is required to reduce the file on disk and needs temporary free space plus an automatic backup.</p></article>
  </section>
 </div>;
}

function StorageBreakdown({rows,databaseBytes}:{rows:DatabaseStorageCategory[];databaseBytes:number}){
 const largest=Math.max(1,...rows.map(row=>row.estimatedPayloadBytes));
 const columns=useMemo<Column<DatabaseStorageCategory>[]>(()=>[
  {key:"category",label:"Category",value:row=>`${row.label} ${row.description}`,render:row=><div className="db-category"><strong>{row.label}</strong><small>{row.description}</small></div>},
  {key:"records",label:"Records",value:row=>row.rowCount,render:row=>row.rowCount.toLocaleString()},
  {key:"estimate",label:"Estimated Payload",value:row=>row.estimatedPayloadBytes,render:row=><div className="db-size"><strong>{formatBytes(row.estimatedPayloadBytes)}</strong><i><b style={{width:`${Math.max(2,row.estimatedPayloadBytes/largest*100)}%`}}/></i></div>},
  {key:"range",label:"Data Range",value:row=>`${row.oldestAt||""} ${row.newestAt||""}`,render:row=><span>{row.oldestAt?`${row.oldestAt.slice(0,10)} → ${row.newestAt?.slice(0,10)||"now"}`:"Current-state data"}</span>},
  {key:"policy",label:"Management",value:row=>row.retentionSupported?"Retention available":"Protected",render:row=><span className={`pill ${row.retentionSupported?"warn":"success"}`}>{row.retentionSupported?"Retention candidate":"Preserve"}</span>},
 ],[largest]);
 const estimated=rows.reduce((sum,row)=>sum+row.estimatedPayloadBytes,0);
 return <section className="db-breakdown"><header><div><h2>Storage breakdown</h2><p>Payload figures are estimates from recent row samples. SQLite indexes and internal page overhead account for the difference from the {formatBytes(databaseBytes)} file.</p></div><span>{formatBytes(estimated)} sampled payload</span></header><DataTable rows={rows} columns={columns} rowKey={row=>row.key} empty="No database categories were found."/></section>;
}

function PreviewResult({value}:{value:DatabaseCleanupPreview}){
 return <div className="db-preview"><article><span>Eligible detail rows</span><strong>{value.totalRows.toLocaleString()}</strong><small>{value.outgoingRows.toLocaleString()} outgoing · {value.incomingRows.toLocaleString()} incoming</small></article><article><span>Estimated payload</span><strong>{formatBytes(value.estimatedPayloadBytes)}</strong><small>Indexes may make actual reusable space larger</small></article><article><span>Cutoff</span><strong>{value.cutoff.slice(0,10)}</strong><small>{value.oldestAt?`${value.oldestAt.slice(0,10)} through ${value.newestAt?.slice(0,10)}`:"No eligible detail"}</small></article><article><span>Summaries</span><strong>{value.encounterSummariesPreserved?"Preserved":"Affected"}</strong><small>Deletion remains disabled pending your policy decision</small></article></div>;
}

function isBackupResult(value:unknown):value is {path:string}{return typeof value==="object"&&value!==null&&"path" in value&&typeof (value as {path?:unknown}).path==="string"}