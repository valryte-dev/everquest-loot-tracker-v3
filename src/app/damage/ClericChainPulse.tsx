import {useMemo,useState} from "react";
import type {TimedClericHealCall} from "./model";
import {buildChainPulse} from "./clericCadence";
import {ChSecondsField} from "./ChSecondsField";

const seconds=(value:number|null)=>value===null?"--":value.toFixed(1)+"s";

export function ClericChainPulse({calls,currentGap,expectedGap}:{calls:TimedClericHealCall[];currentGap:number;expectedGap?:number}){
 const pulse=useMemo(()=>buildChainPulse(calls,currentGap,18,expectedGap),[calls,currentGap,expectedGap]);
 const[scaleInput,setScaleInput]=useState("15");
 const requestedScale=Number(scaleInput);
 const max=Number.isFinite(requestedScale)&&requestedScale>0?Math.min(600,requestedScale):15;
 const w=720,h=132,left=36,right=18,top=16,bottom=24,plotW=w-left-right,plotH=h-top-bottom;
 const count=Math.max(2,pulse.points.length+1);
 const x=(index:number)=>left+index/(count-1)*plotW;
 const y=(value:number)=>top+plotH-Math.min(max,value)/max*plotH;
 const line=pulse.points.map((point,index)=>`${x(index)},${y(point.gapSeconds)}`).join(" ");
 const projectionX=x(pulse.points.length),last=pulse.points.at(-1),target=pulse.targetSeconds;
 const bandTop=target===null?top:y(target+pulse.toleranceSeconds);
 const bandBottom=target===null?top:y(Math.max(0,target-pulse.toleranceSeconds));
 return <section className={`cleric-chain-pulse ${pulse.openGapTone}`}>
  <header>
   <div><span className="eyebrow">Chain pulse</span><h3>Cadence stability</h3><p>{pulse.usesExpectedGap?"Recent gaps compared with the configured target.":"Recent gaps compared with the chain's learned median rhythm."}</p></div>
   <ChSecondsField className="chain-pulse-scale" compact label="Scale 0 to" ariaLabel="Cadence graph maximum seconds" min={1} max={600} step={1} value={scaleInput} onChange={setScaleInput} onCommit={()=>setScaleInput(String(max))}/>
   <div className="chain-pulse-metrics">
    <div><span>Stability</span><strong>{pulse.stabilityScore===null?"Learning":pulse.stabilityScore+"%"}</strong><small>{pulse.stabilityLabel}</small></div>
    <div><span>{pulse.usesExpectedGap?"Target":"Median"}</span><strong>{seconds(target)}</strong><small>{pulse.usesExpectedGap&&pulse.medianSeconds!==null?`Observed ${seconds(pulse.medianSeconds)}`:`${pulse.points.length} gap${pulse.points.length===1?"":"s"}`}</small></div>
    <div><span>Trend</span><strong>{pulse.trend}</strong><small>{pulse.deviationSeconds===null?"Waiting for rhythm":"± "+pulse.deviationSeconds.toFixed(1)+"s variation"}</small></div>
    <div className="open"><span>Open gap</span><strong>{currentGap.toFixed(1)}s</strong><small>{pulse.openGapTone==="overdue"?"Past the target window":pulse.openGapTone==="due"?"Next call is due":"Building toward next call"}</small></div>
   </div>
  </header>
  <figure>
   <svg data-scale-max={max} viewBox={`0 0 ${w} ${h}`} role="img" aria-label={`Complete Heal cadence chart. ${pulse.stabilityLabel}, target ${seconds(target)}, current gap ${currentGap.toFixed(1)} seconds.`}>
    <line className="axis" x1={left} x2={w-right} y1={y(0)} y2={y(0)}/>
    {[0,.5,1].map(scale=><g key={scale}><line className="grid" x1={left} x2={w-right} y1={y(max*scale)} y2={y(max*scale)}/><text x={left-6} y={y(max*scale)+3} textAnchor="end">{Math.round(max*scale)}s</text></g>)}
    {target!==null&&<><rect className="pace-band" x={left} y={bandTop} width={plotW} height={Math.max(2,bandBottom-bandTop)}/><line className="median" x1={left} x2={w-right} y1={y(target)} y2={y(target)}/></>}
    {pulse.points.length>1&&<polyline className="cadence-line" points={line}/>}
    {pulse.points.map((point,index)=><circle key={`${point.callNumber}-${index}`} className={point.tone} cx={x(index)} cy={y(point.gapSeconds)} r="5"><title>{`Call #${String(point.callNumber).padStart(3,"0")}: ${point.gapSeconds.toFixed(1)}s - ${point.tone}`}</title></circle>)}
    {last&&<line className={`projection ${pulse.openGapTone}`} x1={x(pulse.points.length-1)} y1={y(last.gapSeconds)} x2={projectionX} y2={y(currentGap)}/>}
    <circle className={`now ${pulse.openGapTone}`} cx={projectionX} cy={y(currentGap)} r="6"><title>{`Current open gap: ${currentGap.toFixed(1)}s`}</title></circle>
    <text className="now-label" x={projectionX} y={Math.max(10,y(currentGap)-10)} textAnchor="middle">NOW</text>
   </svg>
   <figcaption><span><i className="stable"/>Within rhythm</span><span><i className="early"/>Early</span><span><i className="late"/>Late</span><span><i className="now"/>Current gap</span></figcaption>
  </figure>
 </section>;
}
