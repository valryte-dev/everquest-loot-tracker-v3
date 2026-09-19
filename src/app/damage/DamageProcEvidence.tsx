import {useRef,useState} from "react";
import type {DamageSpellMetric,ProcEvidenceReport} from "../../shared/contracts";
import {getDamageProcEvidence} from "../../shared/backend";
import {Modal,when} from "../ui";
import {DamageProcBars} from "./DamageProcBars";

const number=(value:number)=>Math.round(value).toLocaleString();
const role=(value:string,rawLine:string)=>({
 "preceding-context":/hit by non-melee/i.test(rawLine)?"Direct-damage clue":"Preceding context",
 landing:"Proc landing",
 confirmation:"Attack confirmation",
 "landing-confirmation":"Landing and confirmation",
 supporting:"Supporting line",
}[value]||"Evidence");
const fileName=(value:string)=>value.split(/[\\/]/).pop()||value;

export function DamageProcEvidence({encounterId,metrics}:{encounterId:number;metrics:DamageSpellMetric[]}){
 const[selected,setSelected]=useState(""),[report,setReport]=useState<ProcEvidenceReport|null>(null),[loading,setLoading]=useState(false),[error,setError]=useState("");
 const request=useRef(0);
 const open=async(playerName:string)=>{
  const id=++request.current;
  setSelected(playerName);setReport(null);setError("");setLoading(true);
  try{const value=await getDamageProcEvidence(encounterId,playerName);if(request.current===id)setReport(value)}
  catch(reason){if(request.current===id)setError(String(reason))}
  finally{if(request.current===id)setLoading(false)}
 };
 const close=()=>{request.current++;setSelected("");setReport(null);setError("");setLoading(false)};
 return <><DamageProcBars metrics={metrics} onSelect={playerName=>void open(playerName)}/>
  {selected&&<Modal title={`${selected} proc evidence`} onClose={close} footer={<button onClick={close}>Close</button>}>
   <section className="proc-evidence-intro"><div><span>Counted occurrences</span><strong>{report?.occurrences.length??"..."}</strong></div><p>These are the messages the parser used to count each weapon proc. Context is loaded from the original log only when this window opens.</p></section>
   {loading&&<div className="proc-evidence-loading"><i/><span>Reading proc evidence...</span></div>}
   {error&&<div className="alert"><span>{error}</span></div>}
   {report&&!report.occurrences.length&&<div className="damage-empty">No stored proc occurrences were found for this player in the encounter.</div>}
   {report&&<div className="proc-evidence-list">{report.occurrences.map((occurrence,index)=><article key={occurrence.id}>
    <header><div><span>Proc {index+1}</span><strong>{occurrence.spellName}</strong><small>{occurrence.casterName} to {occurrence.targetName}</small></div><div><b>{occurrence.directDamage?number(occurrence.directDamage)+" direct damage":"Effect / DoT proc"}</b><time>{when(occurrence.happenedAt)}</time></div></header>
    <div className="proc-evidence-meta"><span>{occurrence.sourceName?`Likely source: ${occurrence.sourceName}`:"Weapon source unknown"}</span><span>{occurrence.evidenceSource==="source-log"?`Live log: ${fileName(occurrence.sourceFile)}`:occurrence.evidenceSource==="retained-events"?"Recovered from retained damage events":"Original supporting lines are no longer available"}</span></div>
    {occurrence.messages.length?<ol>{occurrence.messages.map(message=><li key={message.sourceOffset}><span>{role(message.role,message.rawLine)}</span><code>{message.rawLine}</code></li>)}</ol>:<p className="proc-evidence-missing">The proc count is stored, but its original log messages could not be recovered.</p>}
   </article>)}</div>}
  </Modal>}
 </>;
}
