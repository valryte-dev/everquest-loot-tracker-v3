import {renderToStaticMarkup} from "react-dom/server";
import {describe,expect,it} from "vitest";
import {DamageIncomingBars,IncomingDamageTotal,totalIncomingDps} from "./DamageIncomingBars";

const targets=[
 {name:"Tank",rank:1,totalDamage:600,hitCount:6,maxHit:120,firstDamageAt:"2026-09-07 10:00:00",lastDamageAt:"2026-09-07 10:00:10",contribution:75},
 {name:"Pet",rank:2,totalDamage:200,hitCount:2,maxHit:90,firstDamageAt:"2026-09-07 10:00:02",lastDamageAt:"2026-09-07 10:00:08",contribution:25},
];

describe("incoming damage bars",()=>{
 it("renders a proportional leader bar with player and pet detail",()=>{
  const html=renderToStaticMarkup(<DamageIncomingBars targets={targets} activeCharacter="Tank"/>);
  expect(html).toContain("is-damage-leader");
  expect(html).toContain("width:100%");
  expect(html).toContain("width:33.33333333333333%");
  expect(html).toContain("6 hits · max 120");
  expect(html).toContain("Pet");
  expect(html).toContain("ME");
 });
 it("shows total incoming DPS using the overall encounter clock",()=>{
  expect(totalIncomingDps(800,10)).toBe(80);
  const html=renderToStaticMarkup(<IncomingDamageTotal totalDamage={800} durationSeconds={10}/>);
  expect(html).toContain("800");
  expect(html).toContain("80.0 DPS");
 });
 it("protects a new encounter from division by zero",()=>expect(totalIncomingDps(50,0)).toBe(50));
 it("shows a calm empty state before incoming damage arrives",()=>{
  expect(renderToStaticMarkup(<DamageIncomingBars targets={[]} activeCharacter="Tank"/>)).toContain("No incoming damage yet");
 });
});