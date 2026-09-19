import {useMemo,useState} from "react";
import type {Alias,History,Split} from "../../shared/contracts";
import {IconButton,Modal} from "../ui";
import {splitPayoutCompactMessages,splitPayoutFullDetailMessages} from "./model";

export function SoldItemsFullDetailExport({splits,history,aliases,title,close}:{splits:Split[];history:History[];aliases:Alias[];title:string;close:()=>void}){
 const[mode,setMode]=useState<"compact"|"full">("compact");
 const messages=useMemo(()=>mode==="compact"?splitPayoutCompactMessages(splits,history,aliases,title):splitPayoutFullDetailMessages(splits,history,aliases,title),[splits,history,aliases,title,mode]);
 const[copied,setCopied]=useState<number|null>(null),[failed,setFailed]=useState<number|null>(null);
 const copy=async(message:string,index:number)=>{
  try{await navigator.clipboard.writeText(message);setCopied(index);setFailed(null)}
  catch{setFailed(index);setCopied(null)}
 };
 return <Modal title={title+" - Discord messages"} onClose={close} footer={<button onClick={close}>Close</button>}>
  <div className="sold-detail-export">
   <div className="tabs" role="tablist" aria-label="Sold item export detail">
    <button role="tab" aria-selected={mode==="compact"} className={mode==="compact"?"active":""} onClick={()=>{setMode("compact");setCopied(null);setFailed(null)}}>Compact</button>
    <button role="tab" aria-selected={mode==="full"} className={mode==="full"?"active":""} onClick={()=>{setMode("full");setCopied(null);setFailed(null)}}>Full detail</button>
   </div>
   <p>{mode==="compact"?"Item name, phase, value, and split toons":"Complete phase-aware payout details"} divided into {messages.length} Discord-safe message{messages.length===1?"":"s"}. Copy and paste each message in order.</p>
   <div className="sold-detail-message-list">
    {messages.map((message,index)=><article key={index}>
     <header><div><strong>Message {index+1} of {messages.length}</strong><small>{message.length.toLocaleString()} characters</small></div><span role="status">{copied===index?"Copied":failed===index?"Copy failed":""}</span><IconButton icon="clipboard" label={"Copy payout message "+(index+1)} onClick={()=>void copy(message,index)}/></header>
     <pre>{message}</pre>
    </article>)}
   </div>
  </div>
 </Modal>;
}
