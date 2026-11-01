import {describe,expect,it} from "vitest";
import type {ExperienceGainEvent,ExperienceLevelEvent,ExperienceObservation} from "../../shared/contracts";
import {combinedHistoricalRate,historicalLevelPaces,visualExperienceRate} from "./model";

const level=(id:number,happenedAt:string,value:number,direction:"gained"|"lost"="gained"):ExperienceLevelEvent=>({id,happenedAt,character:"Tester",level:value,direction,sourceFile:"eqlog_Tester.txt"});
const gain=(id:number,happenedAt:string):ExperienceGainEvent=>({id,happenedAt,character:"Tester",groupMembers:["Tester"]});
const observation=(id:number,happenedAt:string,percent:number,levelValue=50):ExperienceObservation=>({id,happenedAt,character:"Tester",level:levelValue,percent,confidence:1,gainEvents:0});

describe("historical XP pace",()=>{
 it("uses consecutive level messages as exact 100 percent anchors",()=>{
  const rows=historicalLevelPaces(
   [level(1,"2026-10-20 10:00:00",50),level(2,"2026-10-20 11:00:00",51)],
   [gain(1,"2026-10-20 10:10:00"),gain(2,"2026-10-20 10:40:00"),gain(3,"2026-10-20 11:00:00")],
  );
  expect(rows).toHaveLength(1);
  expect(rows[0].gainCount).toBe(3);
  expect(rows[0].xpPerGain).toBeCloseTo(100/3);
  expect(rows[0].activeHours).toBeCloseTo(1/6);
 });

 it("caps long gaps so offline time is not counted as active grinding",()=>{
  const rows=historicalLevelPaces(
   [level(1,"2026-10-20 10:00:00",50),level(2,"2026-10-21 10:00:00",51)],
   [gain(1,"2026-10-20 10:05:00"),gain(2,"2026-10-20 10:10:00"),gain(3,"2026-10-21 09:55:00")],
  );
  expect(rows[0].activeHours).toBeCloseTo(.25);
  expect(combinedHistoricalRate(rows)).toBeCloseTo(400);
 });

 it("does not invent a historical rate when XP messages are missing",()=>{
  const rows=historicalLevelPaces(
   [level(1,"2026-10-20 10:00:00",50),level(2,"2026-10-20 10:05:00",51)],
   [],
  );
  expect(rows[0].xpPerHour).toBeUndefined();
  expect(combinedHistoricalRate(rows)).toBeUndefined();
 });

 it("ignores lost levels while retaining valid re-ding anchors",()=>{
  const rows=historicalLevelPaces(
   [level(1,"2026-10-20 10:00:00",50),level(2,"2026-10-20 10:05:00",49,"lost"),level(3,"2026-10-20 10:10:00",50),level(4,"2026-10-20 10:20:00",51)],
   [gain(1,"2026-10-20 10:15:00")],
  );
  expect(rows).toHaveLength(1);
  expect(rows[0].fromLevel).toBe(50);
  expect(rows[0].toLevel).toBe(51);
 });

 it("excludes incomplete levels from the combined historical rate",()=>{
  const complete=historicalLevelPaces(
   [level(1,"2026-10-20 10:00:00",50),level(2,"2026-10-20 10:10:00",51)],
   [gain(1,"2026-10-20 10:05:00")],
  )[0];
  const incomplete=historicalLevelPaces(
   [level(3,"2026-10-20 11:00:00",51),level(4,"2026-10-20 11:10:00",52)],
   [],
  )[0];
  expect(combinedHistoricalRate([complete,incomplete])).toBeCloseTo(600);
 });
});

describe("visual XP pace",()=>{
 it("uses only active adjacent reading intervals",()=>{
  const now=new Date("2026-10-20 11:00:00").getTime();
  const rows=[
   observation(1,"2026-10-20 10:00:00",10),
   observation(2,"2026-10-20 10:05:00",20),
   observation(3,"2026-10-20 10:45:00",70),
   observation(4,"2026-10-20 10:50:00",80),
  ];
  expect(visualExperienceRate(rows,now)).toBeCloseTo(120);
 });

 it("handles a level transition without counting an inactive gap",()=>{
  const now=new Date("2026-10-20 11:00:00").getTime();
  const rows=[
   observation(1,"2026-10-20 10:00:00",95,50),
   observation(2,"2026-10-20 10:05:00",5,51),
   observation(3,"2026-10-20 10:30:00",50,51),
  ];
  expect(visualExperienceRate(rows,now)).toBeCloseTo(120);
 });
});
