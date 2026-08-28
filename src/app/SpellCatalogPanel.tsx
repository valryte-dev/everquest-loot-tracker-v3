import {useCallback,useEffect,useState} from "react";
import {getSpellCatalogStatus,reloadSpellCatalog} from "../shared/backend";
import type {SpellCatalogStatus} from "../shared/contracts";
import {when} from "./ui";

export function SpellCatalogPanel(){
 const[status,setStatus]=useState<SpellCatalogStatus|null>(null),[error,setError]=useState("");
 const refresh=useCallback(()=>getSpellCatalogStatus().then(value=>{setStatus(value);setError("")}).catch(value=>setError(String(value).replace(/^Error:\s*/,""))),[]);
 useEffect(()=>{refresh();const timer=setInterval(refresh,1500);return()=>clearInterval(timer)},[refresh]);
 const reload=async()=>{try{setStatus(await reloadSpellCatalog());setError("")}catch(value){setError(String(value).replace(/^Error:\s*/,""))}};
 return <section className="card spell-catalog-panel">
  <header><div><h2>Spell information catalog</h2><p>The full Project 1999 spell category is cached in a separate local SQLite database.</p></div><div className="card-actions"><button className="primary" disabled={status?.refreshing} onClick={reload}>{status?.refreshing?"Refreshing…":"Reload spell data"}</button></div></header>
  <div className="spell-catalog-status"><article><span>Status</span><strong className={status?.refreshing?"working":""}>{status?.refreshing?"Downloading":"Ready"}</strong></article><article><span>Cached spells</span><strong>{(status?.cachedCount||0).toLocaleString()}</strong></article><article><span>Processed</span><strong>{(status?.processed||0).toLocaleString()}</strong></article><article><span>Saved / skipped</span><strong>{(status?.saved||0).toLocaleString()} / {(status?.failed||0).toLocaleString()}</strong></article></div>
  {status?.refreshing&&<div className="spell-sync-line"><i/><span>Downloading wiki spell templates in batches. You can continue using the app.</span></div>}
  {(error||status?.lastError)&&<div className="spell-sync-error">{error||status?.lastError}</div>}
  <footer>Last completed refresh: {status?.lastRefreshAt?when(status.lastRefreshAt):"Not completed yet"}</footer>
 </section>;
}
