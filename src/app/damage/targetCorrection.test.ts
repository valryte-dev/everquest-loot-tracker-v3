import {describe,expect,it} from "vitest";
import type {DamageEncounter} from "../../shared/contracts";
import {buildTargetCorrectionCandidates} from "./targetCorrection";

const encounter=(id:number,mobName:string,character="Valmezz",myDamage=0,totalDamage=100):DamageEncounter=>({
 id,character,mobName,startedAt:"2026-09-09 10:00:00",lastDamageAt:"2026-09-09 10:00:05",
 totalDamage,meleeDamage:totalDamage,spellDamage:0,hitCount:1,maxHit:totalDamage,outcome:"active",
 sourceFile:"eqlog_"+character+".txt",weapons:[],
 players:myDamage?[{name:character,totalDamage:myDamage,hitCount:1,firstDamageAt:"2026-09-09 10:00:00",lastDamageAt:"2026-09-09 10:00:05"}]:[],
});

describe("live fight target correction candidates",()=>{
 it("offers unique targets from the same character and excludes the current mistaken target",()=>{
  const current=encounter(1,"Treasure Chest");
  const candidates=buildTargetCorrectionCandidates(current,[current,encounter(2,"Grenn","Valmezz",250,500),encounter(3,"grenn","Valmezz",100,900),encounter(4,"Tunare","Other",500,500)]);
  expect(candidates).toEqual([{id:2,mobName:"Grenn",myDamage:250,groupDamage:500}]);
 });

 it("puts the target the current player damaged most first",()=>{
  const current=encounter(1,"Wrong target");
  const candidates=buildTargetCorrectionCandidates(current,[encounter(2,"an add","Valmezz",20,1000),encounter(3,"Grenn","Valmezz",300,400)]);
  expect(candidates.map(candidate=>candidate.mobName)).toEqual(["Grenn","an add"]);
 });
});
