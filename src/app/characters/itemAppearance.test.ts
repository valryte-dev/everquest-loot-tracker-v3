import {describe,expect,it} from "vitest";
import {eqShader,isAdditiveEqShader,itemAppearance} from "./itemAppearance";

describe("item appearance metadata",()=>{
 it("keeps the IT150 orientation correction in the appearance catalog",()=>{
  expect(itemAppearance("it150")).toEqual({
   rotation:{x:Math.PI,y:0,z:0},
  });
  expect(itemAppearance("IT157")).toEqual({animationSpeedScale:.75});
 expect(itemAppearance("IT67")).toBeUndefined();
 });

 it("recognizes additive EQ shaders from Babylon GLTF metadata",()=>{
  expect(eqShader({gltf:{extras:{eqShader:5}}})).toBe(5);
  expect(eqShader({extras:{eqShader:4}})).toBe(4);
  expect(isAdditiveEqShader({gltf:{extras:{eqShader:9}}})).toBe(true);
  expect(isAdditiveEqShader({gltf:{extras:{eqShader:6}}})).toBe(false);
 });
});
