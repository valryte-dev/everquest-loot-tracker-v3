import {describe,expect,it,vi} from "vitest";
import {applyStandardPose,attachmentNodeName,characterAnimationLabel,characterAnimationOptions,customHelmAttachPoint,customHelmModel,headModelVariation,isRobeHeadMaterial,itemEffectAnimationSpeed,modelRoots,playCharacterAnimation} from "./CharacterModelViewer";

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

 it("exposes the real model clips with readable EQSage labels",()=>{
  const clips=[
   {name:"Clone of p01",from:0,to:30},
   {name:"Clone of l02",from:0,to:20},
   {name:"Clone of p01",from:0,to:30},
  ];
  expect(characterAnimationOptions(clips)).toEqual([
   {name:"p01",label:"Idle"},
   {name:"l02",label:"Run"},
  ]);
  expect(characterAnimationLabel("t04")).toBe("Cast - pull back");
 });

 it("loops a selected clip and uses idle as the fallback",()=>{
  const idle={name:"p01",from:0,to:30,play:vi.fn(),goToFrame:vi.fn(),pause:vi.fn(),stop:vi.fn()};
  const run={name:"l02",from:0,to:20,play:vi.fn(),goToFrame:vi.fn(),pause:vi.fn(),stop:vi.fn()};
  expect(playCharacterAnimation([idle,run],"l02")).toBe("l02");
  expect(run.play).toHaveBeenCalledWith(true);
  expect(playCharacterAnimation([idle,run],"missing")).toBe("p01");
  expect(idle.play).toHaveBeenCalledWith(true);
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

 it("maps IT240 to a race and gender custom helm on the real head joint",()=>{
  const valmonkCowl={itemName:"Custom Cowl of Mortality",itemId:2612,location:"Head",idFile:"IT240",material:0} as never;
  expect(headModelVariation(valmonkCowl)).toBe(0);
  expect(headModelVariation({material:240} as never)).toBe(0);
  expect(customHelmModel("ikm",valmonkCowl)).toBe("IT635");
  expect(customHelmModel("ikf",valmonkCowl)).toBe("IT630");
  expect(customHelmAttachPoint()).toBe("he");
  expect(attachmentNodeName("Clone of he")).toBe("he");
 expect(headModelVariation({material:2} as never)).toBe(2);
 expect(headModelVariation()).toBe(0);
 });

 it("keeps online recovery available when a local pack asset is missing",()=>{
  expect(modelRoots(true,true)).toEqual([
   "http://127.0.0.1:8765/model-assets/",
   "https://p99planner.com/models/",
  ]);
 expect(modelRoots(true,false)).toEqual([
   "http://127.0.0.1:8765/model-assets/",
   "https://p99planner.com/models/",
  ]);
  expect(modelRoots(true,true,"http://127.0.0.1:8766/model-assets/")).toEqual([
   "http://127.0.0.1:8766/model-assets/",
   "https://p99planner.com/models/",
  ]);
 });
});
