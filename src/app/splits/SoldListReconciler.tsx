import {useMemo,useState} from "react";
import type {Runner,Split,SplitSaleReconciliationRequest} from "../../shared/contracts";
import {DataTable,money,type Column} from "../ui";
import {normalizeSaleItem,parseSoldList,reconcileSoldList,type SaleReconciliationRow} from "./saleReconciliation";

interface Props{splits:Split[];run:Runner}
interface ReconciledSale{key:string;valuePp:number;note:string}

export function SaleReconciliation({splits,run}:Props){
 const[open,setOpen]=useState(false),[text,setText]=useState(""),[owner,setOwner]=useState(""),[rows,setRows]=useState<SaleReconciliationRow[]>([]),[applying,setApplying]=useState(false),[notice,setNotice]=useState("");
 const owners=useMemo(()=>[...new Map(splits.filter(split=>split.looterName).map(split=>[split.looterName!.toLowerCase(),split.looterName!])).values()].sort((a,b)=>a.localeCompare(b)),[splits]);
 const eligible=useMemo(()=>owner?splits.filter(split=>split.looterName?.toLowerCase()===owner.toLowerCase()):splits,[splits,owner]);
 const parsedCount=parseSoldList(text).length,nonEmptyCount=text.split(/\r?\n/).filter(line=>line.trim()).length,skipped=Math.max(0,nonEmptyCount-parsedCount);
 const assigned=new Set(rows.map(row=>row.matchedKey).filter(Boolean));
 const occurrenceLabels=useMemo(()=>{
  const totals=new Map<string,number>(),seen=new Map<string,number>(),labels=new Map<string,string>();
  eligible.forEach(split=>{const identity=normalizeSaleItem(split.itemName);totals.set(identity,(totals.get(identity)||0)+1)});
  eligible.forEach(split=>{const identity=normalizeSaleItem(split.itemName),occurrence=(seen.get(identity)||0)+1;seen.set(identity,occurrence);const total=totals.get(identity)||1;labels.set(split.key,total>1?`copy ${occurrence} of ${total}`:"")});
  return labels;
 },[eligible]);
 const analyze=()=>{setRows(reconcileSoldList(text,eligible));setNotice("")};
 const changeMatch=(id:number,key:string)=>setRows(current=>current.map(row=>row.id===id?{...row,matchedKey:key,status:key?"likely":row.candidates.length?"ambiguous":"unmatched",confidence:key?1:row.confidence}:row));
 const matches=rows.filter(row=>row.matchedKey),unresolved=rows.length-matches.length;
 const apply=async()=>{if(!matches.length||applying)return;const uniqueKeys=new Set(matches.map(row=>row.matchedKey));if(uniqueKeys.size!==matches.length){setNotice("Each held split item can only be used once.");return}setApplying(true);const sales:SplitSaleReconciliationRequest["sales"]=matches.map(row=>({key:row.matchedKey,valuePp:row.pricePp,note:`Reconciled from ${owner?owner+"'s":"a pasted"} sold-item list (${row.itemText})`}));const response=await run("split.reconcileSales",{sales});setApplying(false);if(response===null){setNotice("The sales could not be applied. No pasted rows were cleared.");return}setNotice(`${sales.length} sale${sales.length===1?"":"s"} recorded and moved to pending payouts.`);setRows([]);setText("")};
 const columns:Column<SaleReconciliationRow>[]=[
  {key:"line",label:"Line",value:row=>row.lineNumber},
  {key:"submitted",label:"Submitted Item",value:row=>row.itemText,render:row=><strong>{row.itemText}</strong>},
  {key:"price",label:"Sale Price",value:row=>row.pricePp,render:row=><strong>{money(row.pricePp)}</strong>},
  {key:"basis",label:"Price Basis",value:()=>"Reported actual sale"},
  {key:"match",label:"Matched Held Item",value:row=>eligible.find(split=>split.key===row.matchedKey)?.itemName||"",render:row=><select aria-label={`Match sold line ${row.lineNumber}`} value={row.matchedKey} onChange={event=>changeMatch(row.id,event.target.value)}><option value="">Do not apply / choose a match</option>{eligible.map(split=><option key={split.key} value={split.key} disabled={assigned.has(split.key)&&split.key!==row.matchedKey}>{split.itemName} - {split.looterName||"holder unknown"}{occurrenceLabels.get(split.key)?` - ${occurrenceLabels.get(split.key)}`:""}</option>)}</select>},
  {key:"holder",label:"Held By",value:row=>eligible.find(split=>split.key===row.matchedKey)?.looterName||""},
  {key:"status",label:"Confidence",value:row=>row.status,render:row=><span className={`pill ${row.matchedKey?(row.status==="exact"?"success":"info"):row.status==="ambiguous"?"warn":"error"}`}>{row.matchedKey?(row.status==="exact"?"Exact":"Confirmed"):(row.status==="ambiguous"?"Review":"No match")}</span>}
 ];
 return <section className={`card split-sale-reconcile${open?" open":""}`}><header><div><h2>Reconcile a sold-item list</h2><p>Paste a seller's price list, review one-to-one matches against held split loot, then move confirmed items to pending payouts.</p></div><button onClick={()=>setOpen(value=>!value)}>{open?"Collapse":"Open reconciler"}</button></header>{open&&<div className="split-reconcile-body"><div className="split-reconcile-input"><label className="field"><span>List owner / current holder</span><select value={owner} onChange={event=>{setOwner(event.target.value);setRows([]);setNotice("")}}><option value="">All holders</option>{owners.map(name=><option key={name}>{name}</option>)}</select></label><label className="field wide"><span>Sold items - one per line</span><textarea maxLength={50000} value={text} onChange={event=>{setText(event.target.value);setRows([]);setNotice("")}} placeholder={"400 - PWC\n950 - devouring darkness\n200 - clay bracelet"}/></label><div className="split-reconcile-actions"><span>{nonEmptyCount?`${parsedCount} readable line${parsedCount===1?"":"s"}${skipped?` - ${skipped} skipped`:""}`:"Paste a list to begin"}</span><button onClick={()=>{setText("");setRows([]);setNotice("")}} disabled={!text&&!rows.length}>Clear</button><button className="primary" onClick={analyze} disabled={!parsedCount||!eligible.length}>Analyze matches</button></div></div>{rows.length>0&&<><div className="split-reconcile-summary"><div><span>Parsed sales</span><strong>{rows.length}</strong></div><div><span>Ready to apply</span><strong>{matches.length}</strong></div><div className={unresolved?"warn":"good"}><span>Needs review</span><strong>{unresolved}</strong></div><div><span>Sale total</span><strong>{money(matches.reduce((sum,row)=>sum+row.pricePp,0))}</strong></div></div><DataTable rows={rows} columns={columns} rowKey={row=>row.id} empty="No readable sold-item lines."/><div className="split-reconcile-commit"><p>Only rows with a selected held item will be changed. Unmatched rows remain untouched.</p><button className="primary" disabled={!matches.length||applying} onClick={()=>void apply()}>{applying?"Recording sales...":`Record ${matches.length} matched sale${matches.length===1?"":"s"}`}</button></div></>}{notice&&<div className={`alert ${notice.includes("recorded")?"success":""}`}><span>{notice}</span><button onClick={()=>setNotice("")} aria-label="Dismiss reconciliation message">×</button></div>}</div>}</section>;
}
