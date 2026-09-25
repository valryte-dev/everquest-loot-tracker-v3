import {describe,expect,it} from "vitest";
import {DPS_CLASSES} from "./model";
import {classesForDpsRace,DPS_RACES,racesForDpsClass,startingStatsFor} from "./startingStats";

describe("P99 starting stats",()=>{
 it("contains every playable race/class combination",()=>{
  expect(DPS_RACES).toHaveLength(13);
  expect(DPS_CLASSES.every(characterClass=>racesForDpsClass(characterClass).length>0)).toBe(true);
  expect(DPS_CLASSES.reduce((count,characterClass)=>count+racesForDpsClass(characterClass).length,0)).toBe(73);
 });
 it("returns the wiki values for an Iksar Monk",()=>{
  expect(startingStatsFor("Iksar","Monk")).toMatchObject({strength:75,stamina:75,agility:100,dexterity:95,wisdom:80,intelligence:75,charisma:55,bonusPoints:20});
 });
 it("excludes invalid combinations",()=>{
  expect(startingStatsFor("Iksar","Paladin")).toBeUndefined();
  expect(classesForDpsRace("Iksar",DPS_CLASSES)).toEqual(["Monk","Necromancer","Shadow Knight","Shaman","Warrior"]);
 });
});
