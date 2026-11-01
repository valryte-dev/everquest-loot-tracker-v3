import {describe,expect,it} from "vitest";
import type {SpawnTimer} from "../../shared/contracts";
import {spawnTimerCategory,spawnWindowRemainingPercent} from "./timerDisplay";

const timer=(overrides:Partial<SpawnTimer>={}):SpawnTimer=>({id:1,character:"Tester",mobName:"a mob",killedAt:"2026-10-29T12:00:00-06:00",originKind:"confirmedKill",respawnAt:"2026-10-29T12:10:00-06:00",respawnSeconds:600,windowKind:"fixed",windowNumber:1,totalWindows:1,matchKind:"zoneDefault",...overrides});

describe("spawn timer grouping",()=>{
 it("keeps fixed respawns in zone timers",()=>expect(spawnTimerCategory(timer())).toBe("zone"));
 it("places explicit spawn ranges in window mobs",()=>expect(spawnTimerCategory(timer({windowStartAt:"2026-10-29T12:09:00-06:00",windowEndAt:"2026-10-29T12:11:00-06:00"}))).toBe("window"));
 it("drains remaining progress across the active spawn window",()=>{
  const window=timer({windowStartAt:"2026-10-29T12:10:00-06:00",windowEndAt:"2026-10-29T12:20:00-06:00",windowKind:"after"});
  expect(spawnWindowRemainingPercent(window,Date.parse("2026-10-29T12:10:00-06:00"))).toBe(100);
  expect(spawnWindowRemainingPercent(window,Date.parse("2026-10-29T12:15:00-06:00"))).toBe(50);
  expect(spawnWindowRemainingPercent(window,Date.parse("2026-10-29T12:09:59-06:00"))).toBeUndefined();
 });
});
