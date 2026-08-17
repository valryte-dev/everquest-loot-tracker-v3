import {describe,expect,it} from "vitest";
import type {MerchantMessage} from "../shared/contracts";
import {identifyArbitrage,scoreArbitrage} from "./arbitrage";

const item=(askingPricePp?:number,marketValuePp?:number,marketCount30d=0)=>({id:1,itemName:"This Item",askingPricePp,marketValuePp,marketCount30d});

describe("merchant arbitrage scoring",()=>{
 it("recognizes buy-and-resell profit from a discounted WTS ask",()=>{
  const result=scoreArbitrage("wts",item(1000,1500,5));
  expect(result).toMatchObject({direction:"buy-resell",potentialProfitPp:500,marginPct:50,confidence:"medium",signal:"strong"});
 });

 it("recognizes sell-to-buyer profit from a premium WTB offer",()=>{
  const result=scoreArbitrage("wtb",item(2000,1500,12));
  expect(result?.direction).toBe("sell-to-buyer");
  expect(result?.potentialProfitPp).toBe(500);
  expect(result?.confidence).toBe("high");
 });

 it("requires both prices and a positive spread, then ranks stronger opportunities first",()=>{
  const messages:MerchantMessage[]=[
   {id:1,happenedAt:"2026-08-17T10:00:00",kind:"wts",speakerName:"Thin",message:"WTS This Item",items:[item(1490,1500,1)]},
   {id:2,happenedAt:"2026-08-17T10:01:00",kind:"wts",speakerName:"Strong",message:"WTS This Item",items:[{...item(700,1500,20),id:2}]},
   {id:3,happenedAt:"2026-08-17T10:02:00",kind:"wts",speakerName:"Overpriced",message:"WTS This Item",items:[{...item(1800,1500,20),id:3}]},
   {id:4,happenedAt:"2026-08-17T10:03:00",kind:"wts",speakerName:"NoPrice",message:"WTS This Item",items:[{...item(undefined,1500,20),id:4}]},
  ];
  const results=identifyArbitrage(messages);
  expect(results.map(result=>result.trader)).toEqual(["Strong","Thin"]);
 });

 it("deduplicates repeated auction spam and keeps the trader's newest quote",()=>{
  const messages:MerchantMessage[]=[
   {id:1,happenedAt:"2026-08-17T10:00:00",kind:"wts",speakerName:"Trader",message:"WTS This Item 1000",items:[item(1000,1500,10)]},
   {id:2,happenedAt:"2026-08-17T10:05:00",kind:"wts",speakerName:"Trader",message:"WTS This Item 900",items:[{...item(900,1500,10),id:2}]},
  ];
  const results=identifyArbitrage(messages);
  expect(results).toHaveLength(1);
  expect(results[0].quotedPricePp).toBe(900);
  expect(results[0].messageId).toBe(2);
 });
});
