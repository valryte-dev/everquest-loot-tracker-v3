import {useMemo,useState} from "react";
import type {AppSnapshot,DeathReport,DeathReportDetail} from "../../shared/contracts";
import {getDeathReportDetails} from "../../shared/backend";
import {DataTable,IconButton,Modal,when,type Column} from "../ui";
import {HistoryAnalytics} from "../history/HistoryCharts";

function fileName(path:string){
 const parts=path.split(/[\\/]/);
 return parts[parts.length-1]||path;
}

function lineParts(rawLine:string){
 const match=/^\[([^\]]+)\]\s*(.*)$/.exec(rawLine);
 return match?{time:match[1],message:match[2]}:{time:"",message:rawLine};
}

export function DeathReportsPage({data,run}:{data:AppSnapshot;run:(action:string,payload?:Record<string,unknown>)=>Promise<unknown>}){
 const[detail,setDetail]=useState<DeathReportDetail|null>(null);
 const[loadingId,setLoadingId]=useState<number|null>(null);
 const[detailError,setDetailError]=useState("");
 const[scanning,setScanning]=useState(false);
 const characters=new Set(data.deathReports.map(report=>report.character));
 const topKiller=useMemo(()=>{
  const counts=new Map<string,number>();
  data.deathReports.forEach(report=>counts.set(report.killerName,(counts.get(report.killerName)||0)+1));
  return [...counts].sort((a,b)=>b[1]-a[1]||a[0].localeCompare(b[0]))[0];
 },[data.deathReports]);
 const analyticsRows=useMemo(()=>data.deathReports.map(report=>({label:report.killerName,happenedAt:report.happenedAt,character:report.character})),[data.deathReports]);
 const columns:Column<DeathReport>[]=[
  {key:"time",label:"Death",value:report=>report.happenedAt,render:report=>when(report.happenedAt)},
  {key:"character",label:"Character",value:report=>report.character},
  {key:"killer",label:"Killed By",value:report=>report.killerName,render:report=><strong className="death-killer">{report.killerName}</strong>},
  {key:"context",label:"Captured Context",value:report=>report.contextCount,render:report=><span>{report.contextCount} preceding lines</span>},
  {key:"source",label:"Log File",value:report=>fileName(report.sourceFile)}
 ];
 const openReport=async(report:DeathReport)=>{
  setLoadingId(report.id);
  setDetailError("");
  try{setDetail(await getDeathReportDetails(report.id))}
  catch(error){setDetailError(String(error))}
  finally{setLoadingId(null)}
 };
 const scan=async()=>{
  setScanning(true);
  try{await run("activityHistory.scan")}
  finally{setScanning(false)}
 };
 return <>
  <section className="death-report-hero">
   <div><span className="eyebrow">Combat forensics</span><h2>Death Reports</h2><p>Each report preserves the 30 complete log entries immediately before the fatal message.</p></div>
   <IconButton icon="refresh" label="Scan all character logs now" className="primary" disabled={scanning} onClick={scan}/>
  </section>
  <div className="death-report-stats">
   <article><span>Total deaths</span><strong>{data.deathReports.length.toLocaleString()}</strong><small>Across stored logs</small></article>
   <article><span>Characters affected</span><strong>{characters.size.toLocaleString()}</strong><small>Distinct characters</small></article>
   <article><span>Most lethal</span><strong>{topKiller?topKiller[0]:"None yet"}</strong><small>{topKiller?String(topKiller[1])+" recorded death"+(topKiller[1]===1?"":"s"):"Waiting for scan"}</small></article>
   <article><span>Last scan</span><strong>{data.settings.death_reports_last_scan_at?when(data.settings.death_reports_last_scan_at):"Pending"}</strong><small>{data.settings.death_report_files_scanned||"0"} character logs checked</small></article>
  </div>
  <section className="card death-analytics-card">
   <header><div><h2>Death analytics</h2><p>Explore lethal enemies, deaths over time, character exposure, and dangerous play windows.</p></div></header>
   <HistoryAnalytics rows={analyticsRows} categoryTitle="Deaths by killer" actorTitle="" showActors={false}/>
  </section>
  {detailError&&<div className="alert"><span>{detailError}</span><button onClick={()=>setDetailError("")} aria-label="Dismiss">x</button></div>}
  <section className="card">
   <header><div><h2>Recorded deaths</h2><p>Filter by character, killer, date, or source log. Select the report icon to inspect the final 30 events.</p></div></header>
   <DataTable rows={data.deathReports} columns={columns} rowKey={report=>report.id} empty={scanning?"Scanning character logs...":"No deaths found yet. The background history scan may still be running."} actions={report=><IconButton icon="external" label={"View death report for "+report.character} disabled={loadingId===report.id} onClick={()=>openReport(report)}/>} />
  </section>
  {detail&&<DeathReportViewer report={detail} close={()=>setDetail(null)}/>}
 </>;
}

function DeathReportViewer({report,close}:{report:DeathReportDetail;close:()=>void}){
 return <Modal title={report.character+" - slain by "+report.killerName} onClose={close} footer={<button onClick={close}>Close report</button>}>
  <section className="death-report-summary">
   <div><span>Character</span><strong>{report.character}</strong></div>
   <div><span>Killed by</span><strong>{report.killerName}</strong></div>
   <div><span>Death recorded</span><strong>{when(report.happenedAt)}</strong></div>
   <div><span>Source</span><strong title={report.sourceFile}>{fileName(report.sourceFile)}</strong></div>
  </section>
  <div className="death-context-heading"><div><span className="eyebrow">Final moments</span><h3>Oldest to newest</h3></div><small>{report.entries.length} preceding entries plus the fatal line</small></div>
  <ol className="death-context">
   {report.entries.map(entry=>{
    const parts=lineParts(entry.rawLine);
    const relative=entry.sequenceNumber-report.entries.length-1;
    return <li key={entry.sequenceNumber} title={entry.rawLine}><b>{relative}</b><time>{parts.time}</time><span>{parts.message}</span></li>;
   })}
   {(()=>{
    const parts=lineParts(report.rawLine);
    return <li className="fatal" title={report.rawLine}><b>0</b><time>{parts.time}</time><span>{parts.message}</span></li>;
   })()}
  </ol>
 </Modal>;
}
