import {describe,expect,it} from "vitest";
import type {CharacterProfile} from "../../shared/contracts";
import {filterRosterCharacters} from "./characterRoster";

const profiles=[
 {character:"Derpscleric",race:"da",gender:"m",classCode:"clr",levelSource:"logs"},
 {character:"Derpsmonk",race:"ik",gender:"m",classCode:"mnk",levelSource:"logs"},
] satisfies CharacterProfile[];

describe("character roster filters",()=>{
 it("combines the character name and class filters",()=>{
  expect(filterRosterCharacters(["Derpscleric","Derpsmonk","Valmonk"],profiles,"derps","mnk")).toEqual(["Derpsmonk"]);
 });

 it("can find characters whose class is not known yet",()=>{
  expect(filterRosterCharacters(["Derpscleric","Valmonk"],profiles,"","unknown")).toEqual(["Valmonk"]);
 });

 it("shows the entire roster when both filters are cleared",()=>{
  expect(filterRosterCharacters(["Derpscleric","Derpsmonk"],profiles,"","")).toEqual(["Derpscleric","Derpsmonk"]);
 });
});