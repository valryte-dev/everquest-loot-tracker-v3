import type {SpawnTimer} from "../../shared/contracts";

export const spawnTimerState=(timer:SpawnTimer,now:number)=>{
 const start=Date.parse(timer.windowStartAt||timer.respawnAt),end=Date.parse(timer.windowEndAt||timer.respawnAt);
 const hasWindow=Boolean(timer.windowStartAt&&timer.windowEndAt);
 const phase=now<start?"upcoming":now<=end?"window":"overdue";
 const windowLabel=timer.totalWindows>1?`Window ${timer.windowNumber} of ${timer.totalWindows}`:hasWindow?"Spawn window":"Fixed respawn";
 return {start,end,hasWindow,phase,windowLabel,target:phase==="upcoming"?start:end};
};

export const spawnTimerTargetAt=(timer:SpawnTimer,now:number)=>spawnTimerState(timer,now).target;

export const spawnTimerCategory=(timer:SpawnTimer):"window"|"zone"=>timer.windowStartAt&&timer.windowEndAt?"window":"zone";

export const spawnWindowRemainingPercent=(timer:SpawnTimer,now:number)=>{
 const state=spawnTimerState(timer,now);
 if(!state.hasWindow||state.phase!=="window")return undefined;
 const duration=Math.max(1,state.end-state.start);
 return Math.max(0,Math.min(100,(state.end-now)/duration*100));
};
