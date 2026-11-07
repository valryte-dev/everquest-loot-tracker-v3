import {describe,expect,it} from "vitest";
import type {ExperienceGainEvent,ExperienceLevelEvent,ExperienceObservation} from "../../shared/contracts";
import {combinedHistoricalRate,currentLevelWindow,historicalLevelPaces,summarizeExperienceContexts,visualExperienceRate} from "./model";

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

describe("current level window",()=>{
 it("excludes every observation and gain from completed levels",()=>{
  const observations=[
   observation(1,"2026-11-02 12:40:00",98,44),
   observation(2,"2026-11-02 12:50:35",100,44),
   observation(3,"2026-11-02 13:00:00",2,45),
   observation(4,"2026-11-02 13:10:00",4,45),
  ];
  const events=[gain(1,"2026-11-02 12:45:00"),gain(2,"2026-11-02 12:50:35"),gain(3,"2026-11-02 13:05:00")];
  const window=currentLevelWindow("Tester",observations,events,[level(1,"2026-11-02 12:50:35",45)]);
  expect(window.level).toBe(45);
  expect(window.observations.map(row=>row.id)).toEqual([4,3]);
  expect(window.gainEvents.map(row=>row.id)).toEqual([3]);
 });

 it("uses the latest transition for de-level and re-level boundaries",()=>{
  const observations=[observation(1,"2026-11-02 13:05:00",20,44),observation(2,"2026-11-02 13:20:00",1,45)];
  const transitions=[level(1,"2026-11-02 13:00:00",44,"lost"),level(2,"2026-11-02 13:15:00",45)];
  const window=currentLevelWindow("Tester",observations,[gain(1,"2026-11-02 13:10:00"),gain(2,"2026-11-02 13:20:00")],transitions);
  expect(window.level).toBe(45);
  expect(window.observations.map(row=>row.id)).toEqual([2]);
  expect(window.gainEvents.map(row=>row.id)).toEqual([2]);
 });

 it("falls back to the latest observed level when transition history is absent",()=>{
  const observations=[observation(1,"2026-11-02 12:00:00",90,44),observation(2,"2026-11-02 13:00:00",5,45),observation(3,"2026-11-02 13:10:00",8,45)];
  const window=currentLevelWindow("Tester",observations,[gain(1,"2026-11-02 12:30:00"),gain(2,"2026-11-02 13:05:00")],[]);
  expect(window.level).toBe(45);
  expect(window.observations.map(row=>row.id)).toEqual([3,2]);
  expect(window.gainEvents.map(row=>row.id)).toEqual([2]);
 });
});

describe("experience context summaries",()=>{
 it("shows a newly observed roster before the next visual gauge reading",()=>{
  const solo={...gain(1,"2026-11-04 06:00:00"),groupMembers:["Utrido"]};
  const grouped={...gain(2,"2026-11-04 06:09:02"),groupMembers:["Utrido","Verosson"]};
  const rows=summarizeExperienceContexts(
   [{event:solo,xp:.75}],
   [solo,grouped],
   event=>`${event.groupMembers.length} players`,
   event=>event.groupMembers.join(", "),
  );
  expect(rows).toEqual([
   expect.objectContaining({label:"1 players",gains:1,correlatedGains:1,xpPerGain:.75}),
   expect.objectContaining({label:"2 players",gains:1,correlatedGains:0,xpPerGain:undefined,detail:"Utrido, Verosson"}),
  ]);
 });
});
