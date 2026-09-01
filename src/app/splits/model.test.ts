import {describe,expect,it} from "vitest";
import {buildSplitPayoutSummary} from "./model";

describe("buildSplitPayoutSummary",()=>{
 it("tracks payments independently per canonical participant",()=>{
  const summary=buildSplitPayoutSummary(
   [{key:"held-1",itemName:"Held Crown",addedAt:"2026-08-01",payoutValuePp:101,attendees:["Alt","Main","Friend"]}],
   [{id:7,itemName:"Sold Robe",valuePp:300,disposition:"sold",payoutStatus:"pending",note:"Tunnel sale",completedAt:"2026-08-02",attendees:["Main","HistoryOnly"],payouts:[{name:"HistoryOnly",paidAt:"2026-08-03"}]},{id:8,itemName:"Paid Belt",valuePp:90,disposition:"sold",payoutStatus:"completed",note:"",completedAt:"2026-08-03",paidAt:"2026-08-04",attendees:["Main"],payouts:[{name:"Main",paidAt:"2026-08-04"}]}],
   [{alias:"Alt",canonical:"Main"}],
  );
  const main=summary.people.find(person=>person.name==="Main")!,historyOnly=summary.people.find(person=>person.name==="HistoryOnly")!;
  expect(main.heldSharePp).toBe(50);expect(main.pendingSharePp).toBe(150);expect(main.paidSharePp).toBe(90);expect(historyOnly.pendingSharePp).toBe(0);expect(historyOnly.paidSharePp).toBe(150);expect(summary.pendingValuePp).toBe(150);expect(summary.paidValuePp).toBe(90);
 });
 it("treats consumed items as terminal without adding them to paid payouts",()=>{
  const summary=buildSplitPayoutSummary([], [{id:2,itemName:"Used",valuePp:20,disposition:"consumed",payoutStatus:"completed",note:"Quest",completedAt:"2026-08-02",paidAt:"2026-08-02",attendees:["A"],payouts:[]}], []);
  expect(summary.consumedValuePp).toBe(20);expect(summary.paidValuePp).toBe(0);expect(summary.people[0].consumedItems).toBe(1);expect(summary.people[0].paidSharePp).toBe(0);
 });
});
