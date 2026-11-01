import {useEffect,useState} from "react";
import type {EqTimeSync} from "../../shared/contracts";
import {formatEqTime,projectedEqMinutes} from "./eqTime";

export function EqTimeClock({sync}:{sync?:EqTimeSync}){
 const[now,setNow]=useState(Date.now());
 useEffect(()=>{const timer=window.setInterval(()=>setNow(Date.now()),1000);return()=>window.clearInterval(timer)},[]);
 if(!sync)return <div className="eq-clock unsynchronized" title="Use /time in EverQuest to synchronize the clock"><span>EQ time</span><strong>--:--</strong></div>;
 const minutes=projectedEqMinutes(sync,now),daytime=minutes>=6*60&&minutes<20*60;
 return <div className="eq-clock" title={`${sync.gameDate} · synchronized from ${sync.character} at ${sync.synchronizedAt}`}><i aria-hidden="true">{daytime?"☀":"☾"}</i><span>EQ time</span><strong>{formatEqTime(minutes)}</strong></div>;
}
