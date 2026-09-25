import {describe,expect,it} from "vitest";
import type {InventoryItem,ResearchCatalogItem} from "../../shared/contracts";
import {buildResearchComponentInventory,buildResearchRecipes} from "./model";

const row=(overrides:Partial<ResearchCatalogItem>):ResearchCatalogItem=>({recipeId:1,className:"Enchanter",level:16,spellName:"Levitate",trivial:"22",researchOnly:false,availability:"Yes",sourceUrl:"wiki",componentId:1,itemName:"Part of Tasarin's Grimoire Pg. 23",iconId:1005,quantity:2,componentKind:"page",valueSamples:0,...overrides});
const item=(character:string,itemName:string,count:number,itemId?:number):InventoryItem=>({character,importedAt:"now",id:Math.random(),location:"Bank",itemName,itemId,count});

describe("spell research readiness",()=>{
 it("combines holdings across characters and respects duplicate quantities",()=>{
  const recipes=buildResearchRecipes([row({})],[item("A","Part of Tasarin's Grimoire Pg. 23",1),item("B","Part of Tasarin's Grimoire Pg. 23",1)]);
  expect(recipes[0]).toMatchObject({ready:true,craftableCopies:1,ownedRequirements:1,totalRequirements:1});
  expect(recipes[0].components[0].holders).toHaveLength(2);
 });
 it("prefers canonical ids while retaining name-only matching",()=>{
  const recipes=buildResearchRecipes([row({itemId:77,itemName:"Rune of Test",quantity:1,componentKind:"rune"})],[item("A","Legacy Name",1,77)]);
  expect(recipes[0].ready).toBe(true);
 });
 it("builds one inventory row with every recipe using a component",()=>{
  const recipes=buildResearchRecipes([row({}),row({recipeId:2,spellName:"Other",componentId:2})],[item("A","Part of Tasarin's Grimoire Pg. 23",2)]);
  const components=buildResearchComponentInventory(recipes);
  expect(components).toHaveLength(1);expect(components[0].recipes).toHaveLength(2);expect(components[0].held).toBe(2);
 });
});