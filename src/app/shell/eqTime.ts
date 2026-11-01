import type {EqTimeSync} from "../../shared/contracts";

const DAY_MINUTES=24*60;
const REAL_MILLISECONDS_PER_EQ_MINUTE=3000;

export function projectedEqMinutes(sync:EqTimeSync,now:number){
 const synchronizedAt=Date.parse(sync.synchronizedAt.includes("T")?sync.synchronizedAt:sync.synchronizedAt.replace(" ","T"));
 const elapsed=Number.isFinite(synchronizedAt)?Math.max(0,now-synchronizedAt):0;
 return (sync.gameMinutes+Math.floor(elapsed/REAL_MILLISECONDS_PER_EQ_MINUTE))%DAY_MINUTES;
}

export function formatEqTime(minutesSinceMidnight:number){
 const normalized=((Math.floor(minutesSinceMidnight)%DAY_MINUTES)+DAY_MINUTES)%DAY_MINUTES;
 const hour24=Math.floor(normalized/60),minute=normalized%60,period=hour24>=12?"PM":"AM";
 const hour12=hour24%12||12;
 return `${hour12}:${String(minute).padStart(2,"0")} ${period}`;
}
