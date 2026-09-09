import {useMemo,type CSSProperties} from "react";
import type {DamageEvent} from "../../shared/contracts";
import {buildMeterTrend,buildRollingDpsTrend} from "./meterModel";
import {damageBarColors} from "./DamageFighterBars";

interface ChartPoint {second:number;values:Record<string,number>}

const number=(value:number)=>Math.round(value).toLocaleString();

function TrendChart({title,sub,unit,points,names}:{title:string;sub:string;unit:string;points:ChartPoint[];names:string[]}){
 const width=300,height=96,left=38,right=8,top=7,bottom=17,plotWidth=width-left-right,plotHeight=height-top-bottom;
 const first=points[0]?.second||0,last=Math.max(first+1,points.at(-1)?.second||1);
 const peak=Math.max(1,...points.flatMap(point=>names.map(name=>point.values[name]||0)));
 const x=(second:number)=>left+(second-first)/(last-first)*plotWidth;
 const y=(value:number)=>top+plotHeight-value/peak*plotHeight;
 const line=(name:string)=>points.map(point=>`${x(point.second)},${y(point.values[name]||0)}`).join(" ");
 const latest=points.at(-1)?.values||{};
 return <article className="meter-trend-card">
  <header><div><strong>{title}</strong><small>{sub}</small></div><b>{number(peak)} {unit}</b></header>
  <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label={`${title} for each fighter`}>
   <title>{`${title} for ${names.join(", ")}`}</title>
   {[0,.5,1].map(mark=><g key={mark}><line x1={left} x2={width-right} y1={y(peak*mark)} y2={y(peak*mark)}/><text x={left-5} y={y(peak*mark)+3} textAnchor="end">{number(peak*mark)}</text></g>)}
   {names.map((name,index)=><polyline key={name} style={{stroke:damageBarColors[index%damageBarColors.length]}} points={line(name)}/>) }
   <text x={left} y={height-4}>{first}s</text><text x={width-right} y={height-4} textAnchor="end">{last}s</text>
  </svg>
  <footer>{names.map((name,index)=><span key={name} title={`${name}: ${(latest[name]||0).toFixed(unit==="DPS"?1:0)} ${unit}`}><i style={{"--trend-color":damageBarColors[index%damageBarColors.length]} as CSSProperties}/><em>{name}</em><b>{unit==="DPS"?(latest[name]||0).toFixed(1):number(latest[name]||0)}</b></span>)}</footer>
 </article>;
}

export function DamageTrendCharts({events,startedAt,names,durationSeconds}:{events:DamageEvent[];startedAt:string;names:string[];durationSeconds:number}){
 const cumulative=useMemo(()=>buildMeterTrend(events,startedAt,names,90).map(point=>({second:point.second,values:point.totals})),[events,startedAt,names.join("|")]);
 const rolling=useMemo(()=>buildRollingDpsTrend(events,startedAt,names,durationSeconds).map(point=>({second:point.second,values:point.dps})),[events,startedAt,names.join("|"),durationSeconds]);
 if(!events.length||!names.length)return null;
 return <section className="meter-trend-grid">
  <TrendChart title="Cumulative damage" sub="Total damage by fighter" unit="DMG" points={cumulative} names={names}/>
  <TrendChart title="Rolling DPS" sub="30-second window - latest 2 minutes" unit="DPS" points={rolling} names={names}/>
 </section>;
}
