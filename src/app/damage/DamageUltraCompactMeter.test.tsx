import {renderToStaticMarkup} from "react-dom/server";
import {describe,expect,it} from "vitest";
import type {DamageEncounter} from "../../shared/contracts";
import type {DamageFighterBarRow} from "./DamageFighterBars";
import {DamageUltraCompactMeter} from "./DamageUltraCompactMeter";

const row:DamageEncounter={id:1,character:"Derpsmonk",mobName:"Derek the Vindicator",startedAt:"2026-10-07 10:00:00",lastDamageAt:"2026-10-07 10:01:00",totalDamage:12000,meleeDamage:11600,spellDamage:400,hitCount:80,maxHit:250,incomingDamage:3000,incomingHitCount:20,procCount:2,totalProcDamage:400,dotDamage:125,outcome:"active",sourceFile:"eqlog_Derpsmonk.txt",weapons:["Tranquil Staff"],players:[]};
const fighter:DamageFighterBarRow={name:"Derpsmonk",rank:1,totalDamage:12000,dps:200,procDps:6.7,combatSeconds:60,contribution:100,incomingDamage:3000,mine:true,effects:{procCount:2,procDirectDamage:275,procDotDamage:125,spellCount:0,spellDirectDamage:0,spellDotDamage:0}};

describe("ultra compact damage meter",()=>{
 it("keeps DPS and total damage prominent while retaining a terse fight summary",()=>{
  const html=renderToStaticMarkup(<DamageUltraCompactMeter row={row} durationSeconds={60} fighters={[fighter]}/>);
  expect(html).toContain("Derek the Vindicator");
  expect(html).toContain("200.0");
  expect(html).toContain("12,000");
  expect(html).toContain("DPS");
  expect(html).toContain("DMG");
  expect(html).toContain("IN</b> 3,000 / 50.0 DPS");
  expect(html).toContain("PROC</b> 2 / 400");
  expect(html).toContain("1m 0s · 100.0% · IN 3,000 · 2P/400");
 });
});