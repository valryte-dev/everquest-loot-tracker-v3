import {renderToStaticMarkup} from "react-dom/server";
import {describe,expect,it} from "vitest";
import type {DamageEncounter} from "../../shared/contracts";
import {DamageProcSpellStack} from "./DamageProcSpellStack";

const row={
 id:1,character:"Tester",mobName:"Target",startedAt:"2026-09-12 10:00:00",lastDamageAt:"2026-09-12 10:00:06",
 totalDamage:120,meleeDamage:0,spellDamage:120,hitCount:1,maxHit:120,outcome:"active",sourceFile:"eqlog_Tester.txt",weapons:[],players:[],
 spellMetrics:[{playerName:"Tester",spellName:"One Hundred Blows",procCount:1,directProcDamage:120,dotDamage:0,dotTickCount:0,procDotDamage:0,totalProcDamage:120}],
 trackedSpells:[{id:1,spellName:"One Hundred Blows",targetName:"Target",casterName:"Tester",sourceKind:"proc",happenedAt:"2026-09-12 10:00:01"}],
} as DamageEncounter;

describe("proc and recognized spell stack",()=>{
 it("always renders top proccers before recognized spells",()=>{
  const html=renderToStaticMarkup(<DamageProcSpellStack row={row}/>);
  expect(html.indexOf("Top proccers")).toBeGreaterThan(-1);
  expect(html.indexOf("Recognized spell activity")).toBeGreaterThan(html.indexOf("Top proccers"));
 });
});
