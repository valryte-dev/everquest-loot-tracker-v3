import {useEffect,useMemo,useRef,useState} from "react";
import type {DotTrainingReport} from "../../shared/contracts";
import {formatCombatClock,rankIncomingTargets} from "./meterModel";
import {buildTrainingReplayFrames,createTrainingReplayTimeline,replayCursorAt} from "./trainingReplay";
import {describeTrainingEventSource} from "./damageSources";
import {DamageFighterBars,type DamageFighterBarRow} from "./DamageFighterBars";
import {DamageIncomingBars,IncomingDamageTotal} from "./DamageIncomingBars";

const number=(value:number)=>Math.round(value).toLocaleString();

export function DotTrainingReplay({report}:{report:DotTrainingReport}){
 const timeline=useMemo(()=>createTrainingReplayTimeline(report),[report]);
 const startMs=timeline[0]?.timeMs||0,endMs=timeline.at(-1)?.timeMs||startMs,durationMs=timeline.length?Math.max(1,endMs-startMs):0;
 const[positionMs,setPositionMs]=useState(0),[cursor,setCursor]=useState(0),[playing,setPlaying]=useState(false),[speed,setSpeed]=useState(2);
 const lastTick=useRef(0);
 useEffect(()=>{setPositionMs(0);setCursor(0);setPlaying(false)},[report]);
 useEffect(()=>{
  if(!playing||!timeline.length)return;
  lastTick.current=performance.now();
  const timer=window.setInterval(()=>{
   const now=performance.now(),delta=now-lastTick.current;lastTick.current=now;
   setPositionMs(current=>Math.min(durationMs,current+delta*speed));
  },50);
  return()=>window.clearInterval(timer);
 },[durationMs,playing,speed,timeline.length]);
 useEffect(()=>{
  if(!timeline.length)return;
  if(positionMs<=0){setCursor(0);return}
  const next=replayCursorAt(timeline,startMs+positionMs);
  setCursor(next);
  if(positionMs>=durationMs&&next>=timeline.length)setPlaying(false);
 },[durationMs,positionMs,startMs,timeline]);
 const seek=(next:number)=>{const bounded=Math.max(0,Math.min(durationMs,next));setPositionMs(bounded);setCursor(bounded<=0?0:replayCursorAt(timeline,startMs+bounded))};
 const step=(direction:1|-1)=>{
  setPlaying(false);
  if(direction>0){const event=timeline[Math.min(cursor,timeline.length-1)];if(event)seek(Math.max(1,event.timeMs-startMs))}
  else {if(cursor<=1){restart();return}const event=timeline[cursor-2];seek(event.timeMs-startMs)}
 };
 const restart=()=>{setPlaying(false);setPositionMs(0);setCursor(0)};
 const frames=useMemo(()=>buildTrainingReplayFrames(report,cursor),[cursor,report]);
 const current=cursor?timeline[cursor-1]:undefined;
 return <section className="dot-replay card">
  <header><div><span className="eyebrow">Interactive simulation</span><h2>Battle replay</h2><p>Watch the parser build the same cumulative colored fighter bars from explicit hits, procs, and calculated DoT ticks.</p></div><div className="dot-replay-clock"><strong>{formatCombatClock(positionMs/1000)}</strong><small>{cursor} of {timeline.length} combat events</small></div></header>
  {timeline.length?<><div className="dot-replay-controls">
   <div className="button-row"><button onClick={restart} disabled={!cursor&&!positionMs} title="Return to the beginning">Restart</button><button onClick={()=>step(-1)} disabled={!cursor} title="Step to the previous event">Back</button><button className="primary" onClick={()=>setPlaying(value=>!value)} disabled={!timeline.length||cursor>=timeline.length}>{playing?"Pause":"Play"}</button><button onClick={()=>step(1)} disabled={cursor>=timeline.length} title="Step to the next event">Step</button></div>
   <label><span>Playback speed</span><select value={speed} onChange={event=>setSpeed(Number(event.target.value))}>{[.5,1,2,4,8,16].map(value=><option key={value} value={value}>{value}x</option>)}</select></label>
  </div><label className="dot-replay-scrubber"><input type="range" min="0" max={Math.max(1,durationMs)} step="100" value={positionMs} onChange={event=>{setPlaying(false);seek(Number(event.target.value))}} aria-label="Replay position"/><span>{formatCombatClock(durationMs/1000)}</span></label>
  <div className="dot-replay-now">{current?(()=>{if(current.direction==="incoming")return <><span className="dot-lab-status incoming">incoming</span><strong>{current.target}</strong><span>took <b>{number(current.damage)}</b> from {current.attacker} via {current.attack}</span></>;const encounter=report.encounters.find(value=>value.id===current.encounterId)!,source=describeTrainingEventSource(encounter,current);return <><span className={`dot-lab-status ${current.sourceKind==="explicit"?"recognized":current.sourceKind}`}>{current.sourceKind}</span><strong>{current.attacker}</strong><span>{current.attack} hit {current.mobName} for <b>{number(current.damage)}</b> - <em title={source.evidence}>{source.label}</em></span></>})():<span>Press Play or Step to begin the parsed battle.</span>}</div>
  <div className="dot-replay-fights">{frames.map(frame=><ReplayFight key={frame.encounterId} frame={frame} character={report.activeCharacter}/>)}</div></>:<div className="dot-lab-empty"><strong>No damage events to replay</strong><p>Add damage lines or recognized DoT landings, then analyze again.</p></div>}
 </section>;
}

type Frame=ReturnType<typeof buildTrainingReplayFrames>[number];
function ReplayFight({frame,character}:{frame:Frame;character:string}){
 const fighters:DamageFighterBarRow[]=frame.fighters.map((fighter,index)=>({
  name:fighter.name,rank:index+1,totalDamage:fighter.totalDamage,dps:fighter.dps,
  combatSeconds:fighter.combatSeconds,contribution:fighter.contribution,
  incomingDamage:fighter.incomingDamage,mine:fighter.name.toLowerCase()===character.toLowerCase(),
  shareDetail:`${fighter.hitCount} events`,
  effects:{procCount:fighter.procCount,procDirectDamage:fighter.procDirectDamage,procDotDamage:fighter.procDotDamage,spellCount:fighter.spellCount,spellDirectDamage:fighter.spellDirectDamage,spellDotDamage:fighter.spellDotDamage},
 }));
 const incomingTargets=rankIncomingTargets(frame.incomingTargets);
 const incomingTotal=incomingTargets.reduce((sum,target)=>sum+target.totalDamage,0);
 return <article className="dot-replay-fight"><header><div><span>Replaying</span><h3>{frame.mobName}</h3></div><div><strong>{number(frame.totalDamage)}</strong><small>damage / {formatCombatClock(frame.elapsedSeconds)}</small></div></header><div className="meter-comparison-grid"><section className="meter-ranking-panel outgoing"><header><div><span>Outgoing</span><strong>Damage dealt</strong></div><b>{number(frame.totalDamage)}</b></header><DamageFighterBars fighters={fighters}/></section><section className="meter-ranking-panel incoming"><header><div><span>Incoming</span><strong>Damage taken</strong></div><IncomingDamageTotal totalDamage={incomingTotal} durationSeconds={frame.elapsedSeconds}/></header><DamageIncomingBars targets={incomingTargets} activeCharacter={character}/></section></div></article>;
}