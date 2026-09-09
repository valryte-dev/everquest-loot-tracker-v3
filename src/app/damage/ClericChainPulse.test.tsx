import {renderToStaticMarkup} from "react-dom/server";
import {describe,expect,it} from "vitest";
import type {TimedClericHealCall} from "./model";
import {ClericChainPulse} from "./ClericChainPulse";

const calls:TimedClericHealCall[]=[
 {id:1,happenedAt:"2026-09-08 10:00:00",character:"Tester",clericName:"Alpha",callNumber:1,channel:"guild",message:"LoF 001 CH",sourceFile:"eqlog_Tester.txt",session:0},
 {id:2,happenedAt:"2026-09-08 10:00:09",character:"Tester",clericName:"Beta",callNumber:2,channel:"guild",message:"LoF 002 CH",sourceFile:"eqlog_Tester.txt",session:0,gapSeconds:9},
];

describe("shared CH cadence graph",()=>{
 it("defaults to a 0-15 second scale and exposes a manual maximum",()=>{
  const html=renderToStaticMarkup(<ClericChainPulse calls={calls} currentGap={5}/>);
  expect(html).toContain('data-scale-max="15"');
  expect(html).toContain('aria-label="Cadence graph maximum seconds"');
  expect(html).toContain('type="number"');
  expect(html).toContain('min="1"');
  expect(html).toContain('max="600"');
  expect(html).toContain('value="15"');
  expect(html).toContain("Scale 0 to");
 });
});