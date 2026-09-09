import {renderToStaticMarkup} from "react-dom/server";
import {describe,expect,it} from "vitest";
import {DamageFighterBars,formatFighterDuration,type DamageFighterBarRow} from "./DamageFighterBars";

const fighter:DamageFighterBarRow={
 name:"Asquatii",rank:1,totalDamage:750,dps:125,combatSeconds:6,contribution:100,
 incomingDamage:42,mine:true,shareDetail:"3 events",
 effects:{procCount:1,procDirectDamage:20,procDotDamage:125,spellCount:1,spellDirectDamage:0,spellDotDamage:625},
};

describe("shared damage fighter bars",()=>{
 it("formats durations with compact units and no leading zero units",()=>{
  expect(formatFighterDuration(0)).toBe("0s");
  expect(formatFighterDuration(10)).toBe("10s");
  expect(formatFighterDuration(310)).toBe("5m 10s");
  expect(formatFighterDuration(3910)).toBe("1h 5m 10s");
  expect(formatFighterDuration(3610)).toBe("1h 0m 10s");
 });
 it("fills rows relative to the total-damage leader",()=>{
  const html=renderToStaticMarkup(<DamageFighterBars fighters={[fighter,{...fighter,name:"Balbazak",rank:2,totalDamage:375,dps:140,mine:false}]}/>);
  expect(html).toContain("is-damage-leader");
  expect(html).toContain("width:100%");
  expect(html).toContain("width:50%");
  expect(html).toContain("50.0% of the leading total damage");
 });
 it("renders the common live and training metrics from one typed row",()=>{
  const html=renderToStaticMarkup(<DamageFighterBars fighters={[fighter]}/>);
  expect(html).toContain("Asquatii");
  expect(html).toContain("6s");
  expect(html).toContain("42");
  expect(html).toContain("INCOMING DMG");
  expect(html).toContain("PROCS 1");
  expect(html).toContain("SPELL 1");
  expect(html).toContain("ME");
 });
});
