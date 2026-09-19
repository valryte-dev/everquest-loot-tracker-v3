import {describe,expect,it} from "vitest";
import type {InventoryItem,QuestCatalogItem} from "../../shared/contracts";
import {buildQuestCards,buildVeliousArmorTypeCards,groupQuestCards} from "./model";

const row=(overrides:Partial<QuestCatalogItem>):QuestCatalogItem=>({
 entryId:1,category:"plane_of_sky",className:"Cleric",questName:"Test of Courage",
 rewardName:"Truewind Earring",componentId:1,itemName:"Ochre Tessera",quantity:1,
 slot:"",faction:"",note:"",sourceUrl:"https://wiki.project1999.com",valueSamples:0,...overrides
});
const inventory=(overrides:Partial<InventoryItem>):InventoryItem=>({
 character:"Cleric",importedAt:"2026-09-11",id:1,location:"Carried",
 itemName:"Ochre Tessera",count:1,...overrides
});

describe("quest readiness model",()=>{
 it("matches canonical ids first and aggregates roster holders",()=>{
  const cards=buildQuestCards(
   [row({itemId:42}),row({componentId:2,itemId:43,itemName:"Sky Emerald",valuePp:100})],
   [inventory({itemId:42,itemName:"Different captured spelling"}),inventory({id:2,character:"Alt",itemId:43,itemName:"Sky Emerald",count:2})]
  );
  expect(cards[0].owned).toBe(2);
  expect(cards[0].ready).toBe(true);
  expect(cards[0].components[1].holders[0].character).toBe("Alt");
  expect(cards[0].estimatedHeldValue).toBe(200);
 });
 it("combines Velious slot entries into one class and faction card",()=>{
  const cards=buildQuestCards([
   row({category:"velious_armor",questName:"Warrior Kael Armor",rewardName:"Head",slot:"Head"}),
   row({entryId:2,componentId:2,category:"velious_armor",questName:"Warrior Kael Armor",rewardName:"Feet",itemName:"Boots",slot:"Feet"})
  ],[]);
  expect(cards).toHaveLength(1);
 expect(cards[0].components).toHaveLength(2);
 });
 it("groups Sky quests by class and Velious armor in faction order without duplicating cards",()=>{
  const cards=buildQuestCards([
   row({entryId:1,className:"Wizard"}),
   row({entryId:2,className:"Cleric",componentId:2}),
   row({entryId:3,componentId:3,category:"velious_armor",className:"Warrior",questName:"Warrior Kael Armor",faction:"Kael"}),
   row({entryId:4,componentId:4,category:"velious_armor",className:"Cleric",questName:"Cleric Thurgadin Armor",faction:"Thurgadin"}),
   row({entryId:5,componentId:5,category:"velious_armor",className:"Druid",questName:"Druid Skyshrine Armor",faction:"Skyshrine"})
  ],[]);
  const sky=groupQuestCards(cards.filter(card=>card.category==="plane_of_sky"),"plane_of_sky");
  expect(sky.map(group=>group.label)).toEqual(["Cleric","Wizard"]);
  const armor=groupQuestCards(cards.filter(card=>card.category==="velious_armor"),"velious_armor");
  expect(armor.map(group=>group.label)).toEqual(["Thurgadin","Skyshrine","Kael"]);
 expect(armor.flatMap(group=>group.cards)).toHaveLength(3);
 });
 it("collapses duplicate class armor requirements into zone and armor-type cards",()=>{
  const cards=buildQuestCards([
   row({entryId:1,componentId:1,category:"velious_armor",className:"Warrior",questName:"Warrior Kael Armor",faction:"Kael",itemName:"Ancient Tarnished Breastplate",slot:"Chest"}),
   row({entryId:2,componentId:2,category:"velious_armor",className:"Cleric",questName:"Cleric Kael Armor",faction:"Kael",itemName:"Ancient Tarnished Breastplate",slot:"Chest"}),
   row({entryId:3,componentId:3,category:"velious_armor",className:"Rogue",questName:"Rogue Kael Armor",faction:"Kael",itemName:"Ancient Tarnished Chain Tunic",slot:"Chest"}),
   row({entryId:4,componentId:4,category:"velious_armor",className:"Wizard",questName:"Wizard Kael Armor",faction:"Kael",itemName:"Ancient Silk Robe",slot:"Chest"})
  ],[]);
  const byType=buildVeliousArmorTypeCards(cards);
  expect(byType.map(card=>card.className)).toEqual(["Plate","Chain","Silk"]);
  expect(byType.find(card=>card.className==="Plate")?.components).toHaveLength(1);
 });
});
