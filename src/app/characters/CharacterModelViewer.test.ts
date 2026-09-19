import {describe,expect,it,vi} from "vitest";
import {applyStandardPose,isRobeHeadMaterial,itemEffectAnimationSpeed} from "./CharacterModelViewer";

describe("character model pose",()=>{
 it("holds the exported standard pose without stopping item effect clocks",()=>{
  const idle={from:4,play:vi.fn(),goToFrame:vi.fn(),pause:vi.fn(),stop:vi.fn()};
  const other={from:0,play:vi.fn(),goToFrame:vi.fn(),pause:vi.fn(),stop:vi.fn()};

  applyStandardPose([idle,other],idle);

  expect(idle.stop).toHaveBeenCalledOnce();
  expect(other.stop).toHaveBeenCalledOnce();
  expect(idle.play).toHaveBeenCalledWith(false);
  expect(idle.goToFrame).toHaveBeenCalledWith(4);
  expect(idle.pause).toHaveBeenCalledOnce();
  expect(other.play).not.toHaveBeenCalled();
 });

 it("uses the reference playback speed for cloned POS effect tracks",()=>{
  const pos={from:0,to:120};
 expect(itemEffectAnimationSpeed(pos)).toBe(.35);
 });

 it("uses chest appearance for race-specific robe hood materials",()=>{
  expect(isRobeHeadMaterial("CLKERM06","erm01")).toBe(true);
  expect(isRobeHeadMaterial("clkErF06","erf01")).toBe(true);
  expect(isRobeHeadMaterial("ermhe0001","erm01")).toBe(false);
  expect(isRobeHeadMaterial("CLKERM06","erm")).toBe(false);
 });
});
