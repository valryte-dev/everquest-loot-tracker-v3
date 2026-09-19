import {describe,expect,it} from "vitest";
import type {History} from "../../shared/contracts";
import {buildSplitPayoutSummary,discordPlayerPotentialPayoutSummary,discordPotentialPayoutBriefSummary,discordPotentialPayoutSummary,discordSoldItemsSummary,groupSplitPeople,soldItemsCompactMessages,soldItemsFullDetailMessages,splitPayoutCompactMessages,splitPayoutFullDetailMessages} from "./model";

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
 it("summarizes outstanding people for a selected subset and excludes paid shares",()=>{
  const selected:History[]=[
   {id:1,itemName:"First",valuePp:300,disposition:"sold",payoutStatus:"pending",note:"",completedAt:"2026-09-09",attendees:["Main","Alt","Friend"],payouts:[{name:"Friend",paidAt:"2026-09-09"}]},
   {id:2,itemName:"Second",valuePp:200,disposition:"sold",payoutStatus:"pending",note:"",completedAt:"2026-09-09",attendees:["Main","Other"],payouts:[]},
  ];
  const summary=buildSplitPayoutSummary([],selected,[{alias:"Alt",canonical:"Main"}]);
  expect(summary.pendingValuePp).toBe(350);
  expect(summary.people.filter(person=>person.pendingSharePp>0).map(person=>[person.name,person.pendingSharePp,person.pendingItems])).toEqual([["Main",250,2],["Other",100,1]]);
  expect(summary.people.find(person=>person.name==="Friend")?.pendingSharePp).toBe(0);
 });
});

describe("groupSplitPeople",()=>{
 it("places each person in the highest-priority group without case-insensitive duplicates",()=>{
  const groups=groupSplitPeople(
   ["Valryte","Bluid"],
   ["bluid","Tonel","Historical","VALRYTE"],
   ["Valryte","Tonel","Newperson","historical","Another"],
  );
  expect(groups.current).toEqual(["Bluid","Valryte"]);
  expect(groups.acrossSplits).toEqual(["Historical","Tonel"]);
  expect(groups.others).toEqual(["Another","Newperson"]);
 });
 it("filters each group without changing its membership priority",()=>{
  expect(groupSplitPeople(["Valryte"],["Tonel"],["Other"],"to")).toEqual({current:[],acrossSplits:["Tonel"],others:[]});
 });
});
describe("discordPotentialPayoutSummary",()=>{
 it("formats held estimates from the alias-consolidated payout model",()=>{
  const summary=buildSplitPayoutSummary(
   [{key:"held-1",itemName:"A White Crown",mobName:"a mortiferous golem",looterName:"Holder",addedAt:"2026-08-01",payoutValuePp:101,attendees:["Alt","Main","Friend"]}],
   [],
   [{alias:"Alt",canonical:"Main"}],
  );
  const message=discordPotentialPayoutSummary(summary);
  expect(message).toContain("**Potential Split Payouts**");
  expect(message).toContain("**Main - 50 pp potential**");
  expect(message).toContain("A White Crown - **50 pp share** (101 pp / 2)");
  expect(message).toContain("held by Holder; dropped by a mortiferous golem");
  expect(message).not.toContain("**Alt -");
  const brief=discordPotentialPayoutBriefSummary(summary);
  expect(brief).toContain("- **Main:** 50 pp");
  expect(brief).toContain("- **Friend:** 50 pp");
  expect(brief).not.toContain("A White Crown");
  const main=summary.people.find(person=>person.name==="Main")!;
  const individual=discordPlayerPotentialPayoutSummary(main);
  expect(individual).toContain("**Main - Potential Split Payout**");
  expect(individual).toContain("A White Crown - **50 pp share**");
  expect(individual).not.toContain("**Friend");
 });
 it("provides a useful empty-state message",()=>{
  expect(discordPotentialPayoutSummary(buildSplitPayoutSummary([],[],[]))).toContain("No unsold split loot");
 });
});

describe("discordSoldItemsSummary",()=>{
 it("copies only sold items with alias-consolidated names and the matching per-person value",()=>{
  const message=discordSoldItemsSummary([
   {id:1,itemName:"Peacebringer",valuePp:900,disposition:"sold",payoutStatus:"pending",note:"",completedAt:"2026-09-09",attendees:["Alt","Main","Friend"],payouts:[]},
   {id:2,itemName:"Quest Piece",valuePp:50,disposition:"consumed",payoutStatus:"completed",note:"",completedAt:"2026-09-09",attendees:["Main"],payouts:[]},
  ],[{alias:"Alt",canonical:"Main"}]);
  expect(message).toBe("- **Peacebringer** - Sale price: **900 pp** - Split names: Main, Friend - Split value: **450 pp each**");
  expect(message).not.toContain("Quest Piece");
 });
 it("breaks every full-detail row into numbered Discord-safe messages",()=>{
  const rows:History[]=Array.from({length:240},(_,index)=>({id:index,itemName:"Sold Item "+index,valuePp:901,disposition:"sold",payoutStatus:"pending",note:index===239?"final row":"",completedAt:"2026-09-09 13:00:00",mobName:"Grenn",looterName:"Seller",attendees:["Alt","Main","Friend"],payouts:[{name:"Friend",paidAt:"2026-09-09"}]}));
  const messages=soldItemsFullDetailMessages(rows,[{alias:"Alt",canonical:"Main"}]),message=messages.join("\n");
  expect(messages.length).toBeGreaterThan(1);
  expect(messages.every(part=>part.length<=1900)).toBe(true);
  expect(messages[0]).toContain("**240 items | 216,240 pp total sale value**");
  expect(message).toContain("Player payouts: Main [pending], Friend [paid]");
  expect(message).toContain("Sale value: **901 pp** - Each payout: **450 pp**");
  expect(message).toContain("- **Sold Item 239** - Dropped by: Grenn - Held / sold by: Seller");
  expect(message).toContain("final row");
 });
 it("creates compact Markdown messages containing only item, sale price, and canonical split toons",()=>{
  const rows:History[]=Array.from({length:240},(_,index)=>({id:index,itemName:"Sold Item "+index,valuePp:901,disposition:"sold",payoutStatus:"pending",note:"private note",completedAt:"2026-09-09",mobName:"Grenn",looterName:"Seller",attendees:["Alt","Main","Friend"],payouts:[]}));
  const messages=soldItemsCompactMessages(rows,[{alias:"Alt",canonical:"Main"}]),message=messages.join("\n");
  expect(messages.length).toBeGreaterThan(1);
  expect(messages.every(part=>part.length<=1900)).toBe(true);
  expect(messages[0]).toContain("**Sold Split Items - Compact**");
  expect(messages[0]).toContain("**240 items | 216,240 pp total sale value**");
  expect(message).toContain("- **Sold Item 239** - Sale price: **901 pp** - Split toons: Main, Friend");
  expect(message).not.toContain("Dropped by:");
  expect(message).not.toContain("private note");
 });
});

describe("phase-aware payout Discord messages",()=>{
 it("combines held, pending, and paid records while excluding consumed items",()=>{
  const held=[{key:"held:1",itemName:"Held Crown",addedAt:"2026-09-10",payoutValuePp:1200,attendees:["Alt","Friend"]}];
  const history:History[]=[
   {id:1,itemName:"Pending Robe",valuePp:900,disposition:"sold",payoutStatus:"pending",note:"Tunnel",completedAt:"2026-09-11",attendees:["Alt","Friend"],payouts:[]},
   {id:2,itemName:"Paid Staff",valuePp:600,disposition:"sold",payoutStatus:"completed",note:"Done",completedAt:"2026-09-12",paidAt:"2026-09-13",attendees:["Alt"],payouts:[{name:"Alt",paidAt:"2026-09-13"}]},
   {id:3,itemName:"Consumed Gem",valuePp:50,disposition:"consumed",payoutStatus:"completed",note:"Quest",completedAt:"2026-09-12",attendees:["Alt"],payouts:[]},
  ];
  const aliases=[{alias:"Alt",canonical:"Main"}];
  const compact=splitPayoutCompactMessages(held,history,aliases,"All split phases").join("\n");
  const full=splitPayoutFullDetailMessages(held,history,aliases,"All split phases").join("\n");
  expect(compact).toContain("Phase: **held**");expect(compact).toContain("Phase: **pending**");expect(compact).toContain("Phase: **paid**");
  expect(compact).toContain("Split toons: Main, Friend");expect(compact).not.toContain("Consumed Gem");
  expect(full).toContain("Main [potential]");expect(full).toContain("Main [pending]");expect(full).toContain("Main [paid]");
 });
});
