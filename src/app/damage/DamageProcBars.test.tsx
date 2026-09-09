import {renderToStaticMarkup} from "react-dom/server";
import {describe,expect,it} from "vitest";
import {DamageProcBars} from "./DamageProcBars";

const metrics=[
 {playerName:"Judoku",spellName:"Essence Tap",procCount:2,directProcDamage:80,dotDamage:0,dotTickCount:0,procDotDamage:0,totalProcDamage:80},
 {playerName:"Judoku",spellName:"Dawncall",procCount:1,directProcDamage:0,dotDamage:250,dotTickCount:2,procDotDamage:250,totalProcDamage:250},
 {playerName:"Balbazak",spellName:"Dawncall",procCount:1,directProcDamage:0,dotDamage:125,dotTickCount:1,procDotDamage:125,totalProcDamage:125},
];

describe("damage proc leader bars",()=>{
 it("ranks players by calculated proc damage and displays its composition",()=>{
  const html=renderToStaticMarkup(<DamageProcBars metrics={metrics}/>);
  expect(html.indexOf("Judoku")).toBeLessThan(html.indexOf("Balbazak"));
  expect(html).toContain("3 procs");
  expect(html).toContain("DD 80 / DoT 250");
  expect(html).toContain("330 DMG");
 });
});