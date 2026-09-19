import {describe,expect,it} from "vitest";
import {animatedItemModel,itemParticleDefinitions,itemParticleRenderSettings,itemParticleStartDelay} from "./itemParticleCatalog";

describe("animated item particle catalog",()=>{
 it("covers every epic model with authoritative particle effects",()=>{
  const particleEpics=["IT140","IT141","IT142","IT145","IT146","IT148","IT150","IT151","IT153","IT154","IT155","IT156","IT160"];
  for(const model of particleEpics)expect(itemParticleDefinitions(model).length,model).toBeGreaterThan(0);
  const cleric=itemParticleDefinitions("IT156");
  expect(cleric).toHaveLength(1);
  expect(cleric[0]).toMatchObject({sprite:"grstar1",bones:["it156_p1"],color:[0,0,.588]});
  // Enchanter epic motion is skeletal rather than particle-driven.
  expect(itemParticleDefinitions("IT157")).toEqual([]);
 });

 it("retains the approved Nature Walker leaf calibration",()=>{
  const effect=itemParticleDefinitions("IT150")[0];
  expect(itemParticleRenderSettings(effect,"IT150")).toMatchObject({
   capacity:12,emitRate:40,displayScale:.35,
   minSize:.005,maxSize:.0079,sizeMultiplier:.5,
   minLifeTime:.0675,maxLifeTime:.1125,
   minEmitPower:19.5,maxEmitPower:48.75,
  });
  expect([0,1,2,3,4].map(index=>itemParticleStartDelay("IT150",index,5,40))).toEqual([0,5,10,15,20]);
  expect(itemParticleStartDelay("IT156",2,5,40)).toBe(0);
 });

 it("normalizes directed effects and model aliases like P99 Planner",()=>{
  expect(animatedItemModel("it10638")).toBe("IT24");
  const settings=itemParticleRenderSettings({
   sprite:"test",rate:20,scale:.4,velocity:2,radius:0,lifespan:700,
   count:80,color:[0,0,.588],movement:3,normal:[0,0,2],bones:["point"],
  },"IT999");
  expect(settings).toMatchObject({
   capacity:60,emitRate:50,minSize:.2,maxSize:.2,sizeMultiplier:1,
   minLifeTime:.7,maxLifeTime:.7,minEmitPower:2,maxEmitPower:2,
   direction:[-1,0,0],
  });
 });
});
