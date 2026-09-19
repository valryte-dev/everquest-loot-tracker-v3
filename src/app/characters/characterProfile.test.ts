import {describe,expect,it} from "vitest";
import {characterClassesForRace,characterRacesForClass,isCharacterRaceClassCompatible,normalizeCharacterModelProfile,parseCharacterModelProfile} from "./characterProfile";

describe("Project 1999 race and class compatibility",()=>{
 it("limits race choices for a selected class",()=>{
  expect(characterRacesForClass("shm").map(entry=>entry.name)).toEqual(["Barbarian","Iksar","Ogre","Troll"]);
  expect(characterRacesForClass("mnk").map(entry=>entry.name)).toEqual(["Human","Iksar"]);
 });

 it("limits class choices for a selected race while retaining Unknown",()=>{
  expect(characterClassesForRace("og").map(entry=>entry.name)).toEqual(["Unknown","Warrior","Shadow Knight","Shaman"]);
  expect(characterClassesForRace("hi").map(entry=>entry.name)).toEqual(["Unknown","Cleric","Paladin","Wizard","Magician","Enchanter"]);
 });

 it("recognizes valid and invalid combinations",()=>{
  expect(isCharacterRaceClassCompatible("ik","mnk")).toBe(true);
  expect(isCharacterRaceClassCompatible("ik","pal")).toBe(false);
  expect(isCharacterRaceClassCompatible("ik","")).toBe(true);
 });
});

describe("character model profile",()=>{
 it("restores supported compatible race and gender choices",()=>{
  expect(parseCharacterModelProfile('{"race":"ik","gender":"f","classCode":"shm"}')).toEqual({race:"ik",gender:"f",classCode:"shm"});
 });

 it("clears an incompatible saved class without changing the race",()=>{
  expect(normalizeCharacterModelProfile({race:"og",gender:"m",classCode:"wiz"})).toEqual({race:"og",gender:"m",classCode:""});
 });

 it("falls back safely for corrupt or unsupported settings",()=>{
  expect(parseCharacterModelProfile("broken")).toEqual({race:"hu",gender:"m",classCode:""});
  expect(parseCharacterModelProfile('{"race":"vah","gender":"x","classCode":"beast"}')).toEqual({race:"hu",gender:"m",classCode:""});
 });
});
