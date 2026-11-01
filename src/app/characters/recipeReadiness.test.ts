import {describe,expect,it} from "vitest";
import type {InventoryItem} from "../../shared/contracts";
import {buildRecipeReadiness} from "./recipeReadiness";

const item=(character:string,itemName:string,count=1):InventoryItem=>({id:1,character,itemName,count,location:"Bank",importedAt:"2026-10-28T00:00:00Z"});

describe("character recipe readiness",()=>{
 it("identifies a toon that personally holds every recipe material",()=>{
  const result=buildRecipeReadiness(["Part A","Part B"],[item("Complete","Part A"),item("Complete","Part B"),item("Partial","Part A")]);
  expect(result.completeToons).toEqual([{character:"Complete",craftableCopies:1}]);
  expect(result).toMatchObject({missing:[],craftableCopies:1});
 });
 it("does not claim one toon has all mats when readiness is split across toons",()=>{
  const result=buildRecipeReadiness(["Part A","Part B"],[item("Alpha","Part A"),item("Beta","Part B")]);
  expect(result.completeToons).toEqual([]);
  expect(result).toMatchObject({missing:[],craftableCopies:1});
 });
 it("respects repeated recipe materials and reports multiple combines",()=>{
  const result=buildRecipeReadiness(["Silk","Silk","Rune"],[item("Crafter","Silk",4),item("Crafter","Rune",2)]);
  expect(result.completeToons).toEqual([{character:"Crafter",craftableCopies:2}]);
  expect(result.craftableCopies).toBe(2);
 });
});
