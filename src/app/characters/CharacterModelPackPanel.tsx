import {useCallback,useEffect,useState} from "react";
import type {ModelPackStatus,Runner} from "../../shared/contracts";
import {getModelPackStatus} from "../../shared/backend";
import {PathPicker,when} from "../ui";

const formatBytes=(bytes:number)=>{
 if(bytes<1024)return `${bytes} B`;
 const units=["KB","MB","GB"];let value=bytes/1024,index=0;
 while(value>=1024&&index<units.length-1){value/=1024;index++}
 return `${value.toFixed(value>=100?0:1)} ${units[index]}`;
};

export function CharacterModelPackPanel({run}:{run:Runner}){
 const[status,setStatus]=useState<ModelPackStatus|null>(null);
 const[path,setPath]=useState("");
 const[busy,setBusy]=useState("");
 const[message,setMessage]=useState("");
 const refresh=useCallback(async()=>{
  try{const next=await getModelPackStatus();setStatus(next);if(next.path)setPath(next.path)}
  catch(error){setMessage(String(error))}
 },[]);
 useEffect(()=>{void refresh()},[refresh]);
 const act=async(action:string,payload:Record<string,unknown>={},success:string)=>{
  setBusy(action);setMessage("");
  try{const result=await run(action,payload);if(result){await refresh();setMessage(success)}}
  finally{setBusy("")}
 };
 const ready=status?.installed&&status.valid;
 return <section className="card model-pack-card">
  <header><div><h2>Character model assets</h2><p>Optional external models stay outside SQLite and application releases.</p></div><span className={`pill ${ready?"success":status?.installed?"error":"warn"}`}>{ready?"Ready":status?.installed?"Needs attention":"Remote fallback"}</span></header>
  <div className="model-pack-body">
   <dl className="watcher-details">
    <div><dt>Active source</dt><dd>{ready?status?.name:"P99 Planner online fallback"}</dd></div>
    <div><dt>Pack version</dt><dd>{status?.version||"-"}</dd></div>
    <div><dt>Models</dt><dd>{status?.modelCount||0}</dd></div>
    <div><dt>Pack size</dt><dd>{status?.bytes?formatBytes(status.bytes):"-"}</dd></div>
    <div><dt>Verified</dt><dd>{status?.verifiedAt?when(status.verifiedAt):"-"}</dd></div>
    <div><dt>Location</dt><dd className="model-pack-path">{status?.path||"No local pack connected"}</dd></div>
   </dl>
   {status?.error&&<p className="update-error">{status.error}</p>}
   <div className="model-pack-actions">
    <div><strong>Download appearance pack</strong><p>Downloads all 26 classic race/gender bodies, default heads, and their textures directly from P99 Planner. Existing local data is replaced only after the new pack verifies.</p><button className="primary" disabled={!!busy} onClick={()=>act("modelPack.download",{},"Character model pack downloaded and verified.")}>{busy==="modelPack.download"?"Downloading...":"Download appearance pack"}</button></div>
    <div><strong>Use an external pack folder</strong><p>Select a folder containing model-pack.json. Every listed file is checksum-verified before activation.</p><div className="input-action"><PathPicker value={path} onChange={setPath} kind="folder" placeholder="Choose a model pack folder"/><button disabled={!!busy||!path.trim()} onClick={()=>act("modelPack.activate",{path},"External model pack verified and activated.")}>Use pack</button></div></div>
   </div>
   <div className="button-row">
    <button disabled={!!busy||!status?.installed} onClick={()=>act("modelPack.verify",{},"All model pack checksums are valid.")}>Verify files</button>
    <button disabled={!!busy||!status?.installed} onClick={()=>{if(confirm("Disconnect this model pack? Its files will be preserved."))void act("modelPack.disconnect",{},"Model pack disconnected. Files were preserved.")}}>Disconnect</button>
   </div>
   {message&&<p className="notice" role="status">{message}</p>}
   <p className="muted">The viewer automatically falls back to online models if a local asset cannot be loaded. Downloaded content remains subject to its source's terms.</p>
  </div>
 </section>;
}
