import {useState} from "react";
import type {DamageEncounter,Runner} from "../../shared/contracts";
import {buildTargetCorrectionCandidates} from "./targetCorrection";

const number=(value:number)=>Math.round(value).toLocaleString();

export function FightTargetCorrection({row,encounters,run,close,manual}:{row:DamageEncounter;encounters:DamageEncounter[];run:Runner;close:()=>void;manual:()=>void}){
 const[working,setWorking]=useState<string|null>(null),[error,setError]=useState("");
 const candidates=buildTargetCorrectionCandidates(row,encounters);
 const correct=async(candidate:{id:number;mobName:string})=>{
  setWorking(candidate.id+":"+candidate.mobName);setError("");
  const response=await run("damageTracker.correctTarget",{id:row.id,mobName:candidate.mobName});
  setWorking(null);
  if(response!==null)close();else setError("Target correction failed. The encounter may have already closed.");
 };
 return <section className="damage-quick-target" aria-label={"Correct target for "+row.mobName}>
  <div className="damage-quick-target-heading"><div><span>Correct this fight</span><strong>Choose the actual mob</strong></div><button onClick={close} aria-label="Close target choices" title="Close target choices">x</button></div>
  <div className="damage-target-card-list">
   <article className="current" aria-current="true"><span>Currently inferred</span><strong title={row.mobName}>{row.mobName}</strong><small>Leave unchanged</small></article>
   {candidates.map(candidate=><button key={candidate.id+":"+candidate.mobName.toLowerCase()} disabled={working!==null} onClick={()=>void correct(candidate)} title={"Correct this fight to "+candidate.mobName}><span>{candidate.source==="fighter"?"Observed fighter":"Possible target"}</span><strong>{candidate.mobName}</strong><small>{candidate.source==="fighter"?number(candidate.groupDamage)+" damage in this fight":number(candidate.myDamage)+" mine / "+number(candidate.groupDamage)+" group"}</small></button>)}
   <button className="manual" disabled={working!==null} onClick={manual} title="Enter a different target name"><span>Not listed?</span><strong>Other target</strong><small>Type the mob name</small></button>
  </div>
  {working!==null&&<small className="damage-target-saving">Correcting and merging fight data...</small>}
  {error&&<small className="error-text">{error}</small>}
 </section>;
}
