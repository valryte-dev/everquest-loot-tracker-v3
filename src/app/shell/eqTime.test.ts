import {describe,expect,it} from "vitest";
import {formatEqTime,projectedEqMinutes} from "./eqTime";

const sync={gameMinutes:360,synchronizedAt:"2026-10-28 08:53:48",gameDate:"Tuesday, September 17, 3253",character:"Utrido"};

describe("EQ clock",()=>{
 it("advances one EQ minute every three real seconds",()=>{
  const started=Date.parse("2026-10-28T08:53:48");
  expect(projectedEqMinutes(sync,started+9000)).toBe(363);
 });
 it("formats midnight, morning, noon, and evening",()=>{
  expect(formatEqTime(0)).toBe("12:00 AM");
  expect(formatEqTime(360)).toBe("6:00 AM");
  expect(formatEqTime(720)).toBe("12:00 PM");
  expect(formatEqTime(23*60+59)).toBe("11:59 PM");
 });
 it("wraps cleanly into the next EQ day",()=>{
  const started=Date.parse("2026-10-28T08:53:48");
  expect(projectedEqMinutes({...sync,gameMinutes:1439},started+3000)).toBe(0);
 });
});
