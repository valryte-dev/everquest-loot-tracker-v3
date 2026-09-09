import {describe,expect,it} from "vitest";
import type {TimedClericHealCall} from "./model";
import {buildChainPulse} from "./clericCadence";
const calls=(gaps:number[]):TimedClericHealCall[]=>[{id:1,happenedAt:"2026-09-08 10:00:00",character:"Tester",clericName:"A",callNumber:1,channel:"guild",message:"",sourceFile:"",session:0},...gaps.map((gap,index)=>({id:index+2,happenedAt:`2026-09-08 10:00:${String(index+1).padStart(2,"0")}`,character:"Tester",clericName:index%2?"A":"B",callNumber:index+2,channel:"guild" as const,message:"",sourceFile:"",session:0,gapSeconds:gap}))];
describe("CH chain pulse",()=>{
 it("learns a robust median and identifies an overdue open gap",()=>{const pulse=buildChainPulse(calls([9,10,10,11,20]),14);expect(pulse.medianSeconds).toBe(10);expect(pulse.points.at(-1)?.tone).toBe("late");expect(pulse.openGapTone).toBe("overdue")});
 it("reports a stable cadence and steady trend for consistent gaps",()=>{const pulse=buildChainPulse(calls([9.8,10.1,10,9.9,10.2,10]),5);expect(pulse.stabilityScore).toBeGreaterThan(98);expect(pulse.stabilityLabel).toBe("Locked in");expect(pulse.trend).toBe("Steady");expect(pulse.openGapTone).toBe("on-pace")});
 it("stays in learning mode until a completed gap exists",()=>expect(buildChainPulse(calls([]),3)).toMatchObject({stabilityScore:null,stabilityLabel:"Learning",openGapTone:"learning"}));
});