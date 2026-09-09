import {describe,expect,it} from "vitest";
import type {Split} from "../../shared/contracts";
import {normalizeSaleItem,parseSoldList,reconcileSoldList,scoreSaleItem} from "./saleReconciliation";

const split=(key:string,itemName:string,looterName="Seller"):Split=>({key,itemName,looterName,addedAt:"2026-09-09",attendees:["Seller","Friend"]});

describe("sold-list reconciliation",()=>{
 it("parses flexible price delimiters and preserves duplicate sales",()=>{
  const rows=parseSoldList("400 - PWC\n950- devouring darkness\n1,000 : Tola Robe\n900 devouring darkness");
  expect(rows.map(row=>[row.pricePp,row.itemText])).toEqual([[400,"PWC"],[950,"devouring darkness"],[1000,"Tola Robe"],[900,"devouring darkness"]]);
 });
 it("normalizes spell prefixes, punctuation, possessives, and plurals",()=>{
  expect(normalizeSaleItem("Spell: Divine Interventions")).toBe("divine intervention");
  expect(scoreSaleItem("legacy of thorn","Spell: Legacy of Thorns")).toBe(1);
  expect(scoreSaleItem("clay bracelet","Hardened Clay Bracelet")).toBeGreaterThan(.85);
 });
 it("matches acronyms and assigns repeated names to distinct held records",()=>{
  const held=[split("manual:1","Pulsing Wurm Cultist"),split("manual:2","Spell: Devouring Darkness"),split("manual:3","Spell: Devouring Darkness")];
  const rows=reconcileSoldList("400 - PWC\n950 - devouring darkness\n1000 - Devouring Darkness",held);
  expect(rows.map(row=>row.matchedKey)).toEqual(["manual:1","manual:2","manual:3"]);
  expect(rows.every(row=>row.status==="exact")).toBe(true);
 });
 it("leaves a short ambiguous label unconfirmed",()=>{
  const rows=reconcileSoldList("11500 - Mask",[split("manual:1","Mask of Melodies"),split("manual:2","Mask of the Hunter")]);
  expect(rows[0].matchedKey).toBe("");
  expect(rows[0].status).toBe("ambiguous");
  expect(rows[0].candidates).toHaveLength(2);
 });
 it("does not reuse a held record when the pasted list has more copies",()=>{
  const rows=reconcileSoldList("950 - Devouring Darkness\n1000 - Devouring Darkness",[split("manual:1","Spell: Devouring Darkness")]);
  expect(rows[0].matchedKey).toBe("manual:1");
  expect(rows[1].matchedKey).toBe("");
 });
});