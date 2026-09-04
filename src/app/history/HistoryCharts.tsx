import {useEffect, useMemo, useState, type KeyboardEvent, type MouseEvent, type ReactNode} from "react";
import {activityHeatmap, bucketEvents, cardLootBreakdown, rankCategories, rankValues, type CardColor, type HistoryChartDatum, type TimeGrain} from "./analytics";

const CHART_COLORS = Array.from({length: 11}, (_, index) => `var(--chart-${index + 1})`);
const GRAINS:TimeGrain[] = ["auto","day","week","month","year"];
const DAYS = ["Mon","Tue","Wed","Thu","Fri","Sat","Sun"];
type ChartKey = "frequency"|"timeline"|"value"|"cards"|"actors"|"characters"|"rhythm"|"levels";

function ExpandablePanel({children,expanded=false,onExpand,className=""}:{children:ReactNode;expanded?:boolean;onExpand?:()=>void;className?:string}) {
  const open = (event:KeyboardEvent<HTMLElement>) => {
    if (!onExpand || (event.key !== "Enter" && event.key !== " ")) return;
    event.preventDefault();
    onExpand();
  };
  return <section className={`history-chart-panel ${onExpand?"expandable":""} ${expanded?"expanded":""} ${className}`.trim()} onClick={onExpand} onKeyDown={open} tabIndex={onExpand?0:undefined}>
    {children}
    {onExpand&&<span className="history-expand-hint" aria-hidden="true">↗</span>}
  </section>;
}

function DonutChart({rows,title,expanded,onExpand}:{rows:HistoryChartDatum[];title:string;expanded?:boolean;onExpand?:()=>void}) {
  const slices = useMemo(() => rankCategories(rows), [rows]);
  const total = slices.reduce((sum, slice) => sum + slice.count, 0);
  let offset = 0;
  return <ExpandablePanel expanded={expanded} onExpand={onExpand}>
    <header><div><span className="eyebrow">Most frequent</span><h3>{title}</h3></div><strong>{total.toLocaleString()}</strong></header>
    {!total?<div className="history-chart-empty">No events to chart yet.</div>:<div className="history-donut-layout">
      <svg className="history-donut" viewBox="0 0 120 120" role="img" aria-label={`${title}: ${total} total events`}>
        <circle className="history-donut-track" cx="60" cy="60" r="43" pathLength="100"/>
        {slices.map((slice, index) => {
          const percentage = slice.count / total * 100;
          const currentOffset = offset;
          offset += percentage;
          return <circle key={slice.label} cx="60" cy="60" r="43" pathLength="100" fill="none" stroke={CHART_COLORS[index]} strokeWidth="18" strokeDasharray={`${percentage} ${100-percentage}`} strokeDashoffset={-currentOffset} transform="rotate(-90 60 60)">
            <title>{slice.label}: {slice.count} ({Math.round(percentage)}%)</title>
          </circle>;
        })}
        <text x="60" y="57" textAnchor="middle" className="history-donut-total">{total.toLocaleString()}</text>
        <text x="60" y="70" textAnchor="middle" className="history-donut-caption">events</text>
      </svg>
      <ol className="history-chart-legend">{slices.map((slice,index)=><li key={slice.label}><i style={{background:CHART_COLORS[index]}}/><span title={slice.label}>{slice.label}</span><b>{slice.count.toLocaleString()}</b></li>)}</ol>
    </div>}
  </ExpandablePanel>;
}

function BarChart({rows,grain,setGrain,expanded,onExpand}:{rows:HistoryChartDatum[];grain:TimeGrain;setGrain:(grain:TimeGrain)=>void;expanded?:boolean;onExpand?:()=>void}) {
  const buckets = useMemo(() => bucketEvents(rows,grain), [rows,grain]);
  const maximum = Math.max(0,...buckets.map(bucket=>bucket.count));
  const chartWidth = Math.max(expanded?1040:720,buckets.length*22);
  const height=240,left=42,right=12,top=16,bottom=38,plotWidth=chartWidth-left-right,plotHeight=height-top-bottom;
  const slot=buckets.length?plotWidth/buckets.length:plotWidth;
  const labelEvery=Math.max(1,Math.ceil(buckets.length/(expanded?10:6)));
  const stop=(event:MouseEvent)=>event.stopPropagation();
  return <ExpandablePanel expanded={expanded} onExpand={onExpand}>
    <header><div><span className="eyebrow">Activity timeline</span><h3>Events over time</h3></div><div className="history-grain-picker" onClick={stop}>{GRAINS.map(value=><button key={value} className={grain===value?"active":""} onClick={()=>setGrain(value)}>{value[0].toUpperCase()+value.slice(1)}</button>)}</div></header>
    {!buckets.length?<div className="history-chart-empty">No valid timestamps to chart yet.</div>:<div className="history-bar-scroll"><svg className="history-bars" style={{width:chartWidth}} viewBox={`0 0 ${chartWidth} ${height}`} role="img" aria-label={`Events grouped by ${grain}`}>
      {[0,.5,1].map(ratio=>{const y=top+plotHeight*(1-ratio);return <g key={ratio}><line x1={left} x2={chartWidth-right} y1={y} y2={y}/><text x={left-7} y={y+3} textAnchor="end">{Math.round(maximum*ratio)}</text></g>})}
      {buckets.map((bucket,index)=>{const barHeight=maximum?Math.max(2,bucket.count/maximum*plotHeight):0;const barWidth=Math.max(2,slot*.72);const x=left+index*slot+(slot-barWidth)/2;const y=top+plotHeight-barHeight;return <g key={bucket.key}><rect x={x} y={y} width={barWidth} height={barHeight} rx={Math.min(3,barWidth/3)}><title>{bucket.label}: {bucket.count} event{bucket.count===1?"":"s"}</title></rect>{(index%labelEvery===0||index===buckets.length-1)&&<text className="history-bar-label" x={x+barWidth/2} y={height-12} textAnchor="middle">{bucket.label}</text>}</g>})}
    </svg></div>}
  </ExpandablePanel>;
}

function ValueChart({rows,expanded,onExpand}:{rows:HistoryChartDatum[];expanded?:boolean;onExpand?:()=>void}) {
  const values=useMemo(()=>rankValues(rows),[rows]);
  const maximum=Math.max(0,...values.map(value=>value.valuePp));
  const formatPp=(value:number)=>`${value.toLocaleString()} pp`;
  return <ExpandablePanel expanded={expanded} onExpand={onExpand} className="history-value-chart">
    <header><div><span className="eyebrow">Market intelligence</span><h3>Top items by individual value</h3></div><strong>{values.length?`Top ${values.length}`:"—"}</strong></header>
    {!values.length?<div className="history-chart-empty">No priced loot is available to chart yet.</div>:<div className="history-value-bars">{values.map((value,index)=><article key={value.label}><b>{index+1}</b><div className="history-value-copy"><span title={value.label}>{value.label}</span><small>{value.count} recorded drop{value.count===1?"":"s"}</small></div><div className="history-value-track"><i style={{width:`${value.valuePp/maximum*100}%`}}/></div><strong>{formatPp(value.valuePp)}</strong></article>)}</div>}
  </ExpandablePanel>;
}

function RankingChart({rows,title,eyebrow,expanded,onExpand}:{rows:HistoryChartDatum[];title:string;eyebrow:string;expanded?:boolean;onExpand?:()=>void}) {
  const ranked=useMemo(()=>rankCategories(rows),[rows]);
  const maximum=Math.max(0,...ranked.map(item=>item.count));
  return <ExpandablePanel expanded={expanded} onExpand={onExpand}>
    <header><div><span className="eyebrow">{eyebrow}</span><h3>{title}</h3></div><strong>{ranked.reduce((sum,item)=>sum+item.count,0).toLocaleString()}</strong></header>
    {!ranked.length?<div className="history-chart-empty">No activity is available to rank yet.</div>:<div className="history-ranking">{ranked.map((item,index)=><article key={item.label}><b>{index+1}</b><span title={item.label}>{item.label}</span><div><i style={{width:`${item.count/maximum*100}%`,background:CHART_COLORS[index]}}/></div><strong>{item.count.toLocaleString()}</strong></article>)}</div>}
  </ExpandablePanel>;
}

function RhythmChart({rows,expanded,onExpand}:{rows:HistoryChartDatum[];expanded?:boolean;onExpand?:()=>void}) {
  const cells=useMemo(()=>activityHeatmap(rows),[rows]);
  const maximum=Math.max(0,...cells.map(cell=>cell.count));
  const total=cells.reduce((sum,cell)=>sum+cell.count,0);
  return <ExpandablePanel expanded={expanded} onExpand={onExpand} className="history-rhythm-chart">
    <header><div><span className="eyebrow">Play pattern</span><h3>Weekly activity fingerprint</h3></div><strong>{total.toLocaleString()} events</strong></header>
    {!total?<div className="history-chart-empty">No timestamped activity is available yet.</div>:<div className="history-heatmap-scroll"><div className="history-heatmap">
      <span/>
      {Array.from({length:24},(_,hour)=><small key={hour} className={hour%3===0?"show":""}>{hour%3===0?String(hour).padStart(2,"0"):""}</small>)}
      {DAYS.map((day,dayIndex)=><div className="history-heatmap-row" key={day}><b>{day}</b>{cells.filter(cell=>cell.day===dayIndex).map(cell=><i key={cell.hour} style={{opacity:cell.count?Math.max(.14,cell.count/maximum):.035}}><title>{day} {String(cell.hour).padStart(2,"0")}:00–{String((cell.hour+1)%24).padStart(2,"0")}:00 · {cell.count} event{cell.count===1?"":"s"}</title></i>)}</div>)}
    </div></div>}
  </ExpandablePanel>;
}

function CardLootChart({rows,expanded,onExpand}:{rows:HistoryChartDatum[];expanded?:boolean;onExpand?:()=>void}) {
  const icons:Record<string,string>={Thrones:"/cards/Item_651.png",Crowns:"/cards/Item_653.png",Knights:"/cards/Item_654.png",Squires:"/cards/Item_649.png"};
  const groups=useMemo(()=>cardLootBreakdown(rows),[rows]);
  const colors:CardColor[]=["Black","Blue","Red","White"];
  const total=groups.reduce((sum,group)=>sum+group.total,0);
  const variants=groups.reduce((sum,group)=>sum+colors.filter(color=>group.colors[color]>0).length,0);
  const maximum=Math.max(0,...groups.map(group=>group.total));
  return <ExpandablePanel expanded={expanded} onExpand={onExpand} className="history-card-loot-chart">
    <header><div><span className="eyebrow">Special card drops</span><h3>Thrones, Crowns, Knights & Squires</h3></div><strong>{total.toLocaleString()} cards · {variants}/16 variants</strong></header>
    {!total?<div className="history-chart-empty">No special card loot has been discovered yet.</div>:<div className="history-card-chart-body">
      <div className="history-card-legend">{colors.map(color=><span key={color}><i className={color.toLowerCase()}/>{color}</span>)}</div>
      <div className="history-card-bars">{groups.map(group=><article key={group.type}>
        <div className="history-card-label"><img src={icons[group.type]} alt=""/><span><strong>{group.type}</strong><small>{group.total} looted</small></span></div>
        <div className="history-card-track">{colors.map(color=>group.colors[color]?<i key={color} className={color.toLowerCase()} style={{width:`${group.colors[color]/maximum*100}%`}}><title>{color} {group.type}: {group.colors[color]}</title></i>:null)}</div>
        <b>{group.total}</b>
        <p>{colors.map(color=><span key={color} className={`${color.toLowerCase()} ${group.colors[color]?"found":""}`.trim()}><i/>{color} <b>{group.colors[color]}</b></span>)}</p>
      </article>)}</div>
    </div>}
  </ExpandablePanel>;
}

function LevelPathChart({rows,expanded,onExpand}:{rows:HistoryChartDatum[];expanded?:boolean;onExpand?:()=>void}) {
  const events=useMemo(()=>rows.filter(row=>Number.isFinite(row.level)&&Number.isFinite(Date.parse(row.happenedAt))).sort((left,right)=>Date.parse(left.happenedAt)-Date.parse(right.happenedAt)),[rows]);
  const characters=useMemo(()=>[...new Set(events.map(row=>row.character||"Unknown"))],[events]);
  const width=expanded?1180:900,height=330,left=48,right=24,top=24,bottom=42;
  const times=events.map(row=>Date.parse(row.happenedAt));
  const minimumTime=Math.min(...times),maximumTime=Math.max(...times);
  const levels=events.map(row=>row.level||1);
  const minimumLevel=Math.max(1,Math.min(...levels)-1),maximumLevel=Math.max(...levels)+1;
  const x=(time:number)=>minimumTime===maximumTime?left+(width-left-right)/2:left+(time-minimumTime)/(maximumTime-minimumTime)*(width-left-right);
  const y=(level:number)=>top+(maximumLevel-level)/Math.max(1,maximumLevel-minimumLevel)*(height-top-bottom);
  const tickCount=Math.min(6,maximumLevel-minimumLevel+1);
  const ticks=Array.from({length:tickCount},(_,index)=>Math.round(minimumLevel+(maximumLevel-minimumLevel)*index/Math.max(1,tickCount-1))).filter((value,index,array)=>array.indexOf(value)===index);
  const current=characters.map(character=>events.filter(row=>(row.character||"Unknown")===character).at(-1)).filter(Boolean) as HistoryChartDatum[];
  return <ExpandablePanel expanded={expanded} onExpand={onExpand} className="history-level-chart">
    <header><div><span className="eyebrow">Character progression</span><h3>Leveling path</h3></div><div className="history-level-current">{current.map((row,index)=><span key={row.character}><i style={{background:CHART_COLORS[index%CHART_COLORS.length]}}/>{row.character} <b>{row.level}</b></span>)}</div></header>
    {!events.length?<div className="history-chart-empty">No level milestones have been discovered yet.</div>:<div className="history-bar-scroll"><svg className="history-level-path" style={{width}} viewBox={`0 0 ${width} ${height}`} role="img" aria-label="Character levels over time">
      {ticks.map(level=><g key={level}><line x1={left} x2={width-right} y1={y(level)} y2={y(level)}/><text x={left-9} y={y(level)+4} textAnchor="end">L{level}</text></g>)}
      {characters.map((character,index)=>{const characterEvents=events.filter(row=>(row.character||"Unknown")===character);const points=characterEvents.map(row=>`${x(Date.parse(row.happenedAt))},${y(row.level||1)}`).join(" ");return <g key={character}><polyline points={points} fill="none" stroke={CHART_COLORS[index%CHART_COLORS.length]} strokeWidth="3"/>{characterEvents.map((row,eventIndex)=><circle key={eventIndex} cx={x(Date.parse(row.happenedAt))} cy={y(row.level||1)} r={row.direction==="lost"?5:4} fill={row.direction==="lost"?"var(--bad)":CHART_COLORS[index%CHART_COLORS.length]} stroke="var(--panel)" strokeWidth="2"><title>{character} reached level {row.level} · {new Date(row.happenedAt).toLocaleString()}{row.direction==="lost"?" · level lost":""}</title></circle>)}</g>})}
      <text className="history-level-date" x={left} y={height-13}>{new Date(minimumTime).toLocaleDateString()}</text>
      <text className="history-level-date" x={width-right} y={height-13} textAnchor="end">{new Date(maximumTime).toLocaleDateString()}</text>
    </svg></div>}
  </ExpandablePanel>;
}

export function HistoryAnalytics({rows,categoryTitle,actorTitle,showValues=false,showCards=false,levelPath=false}:{rows:HistoryChartDatum[];categoryTitle:string;actorTitle:string;showValues?:boolean;showCards?:boolean;levelPath?:boolean}) {
  const[grain,setGrain]=useState<TimeGrain>("auto");
  const[expanded,setExpanded]=useState<ChartKey|null>(null);
  useEffect(()=>{if(!expanded)return;const close=(event:globalThis.KeyboardEvent)=>event.key==="Escape"&&setExpanded(null);addEventListener("keydown",close);return()=>removeEventListener("keydown",close)},[expanded]);
  const actorRows=useMemo(()=>rows.filter(row=>row.actor?.trim()).map(row=>({...row,label:row.actor!.trim()})),[rows]);
  const characterRows=useMemo(()=>rows.filter(row=>row.character?.trim()).map(row=>({...row,label:row.character!.trim()})),[rows]);
  const chart=(key:ChartKey,isExpanded=false)=>{
    const open=isExpanded?undefined:()=>setExpanded(key);
    if(key==="levels")return <LevelPathChart rows={rows} expanded={isExpanded} onExpand={open}/>;
    if(key==="frequency")return <DonutChart rows={rows} title={categoryTitle} expanded={isExpanded} onExpand={open}/>;
    if(key==="timeline")return <BarChart rows={rows} grain={grain} setGrain={setGrain} expanded={isExpanded} onExpand={open}/>;
    if(key==="value")return <ValueChart rows={rows} expanded={isExpanded} onExpand={open}/>;
    if(key==="cards")return <CardLootChart rows={rows} expanded={isExpanded} onExpand={open}/>;
    if(key==="actors")return <RankingChart rows={actorRows} title={actorTitle} eyebrow="People involved" expanded={isExpanded} onExpand={open}/>;
    if(key==="characters")return <RankingChart rows={characterRows} title="Events by logged character" eyebrow="Character footprint" expanded={isExpanded} onExpand={open}/>;
    return <RhythmChart rows={rows} expanded={isExpanded} onExpand={open}/>;
  };
  return <>
    <section className="history-analytics" aria-label="History analytics">
      {chart(levelPath?"levels":"frequency")}
      {chart("timeline")}
      {showValues&&chart("value")}
      {showCards&&chart("cards")}
      {!levelPath&&chart("actors")}
      {chart("characters")}
      {chart("rhythm")}
    </section>
    {expanded&&<div className="history-chart-modal-backdrop" onMouseDown={event=>event.target===event.currentTarget&&setExpanded(null)}><section className="history-chart-modal" role="dialog" aria-modal="true" aria-label="Expanded history chart"><header><div><span className="eyebrow">Expanded analytics</span><strong>History detail view</strong></div><button onClick={()=>setExpanded(null)} aria-label="Close expanded chart">×</button></header><div>{chart(expanded,true)}</div></section></div>}
  </>;
}
