import {renderToStaticMarkup} from "react-dom/server";
import {describe,expect,it} from "vitest";
import {DamageAttackAnalytics} from "./DamageAttackAnalytics";

describe("DamageAttackAnalytics",()=>{
 it("shows melee techniques and a multi-dimensional combat fingerprint",()=>{
  const html=renderToStaticMarkup(<DamageAttackAnalytics metrics={[
   {attack:"slash",damageType:"melee",totalDamage:1200,hitCount:12,maxHit:180},
   {attack:"kick",damageType:"melee",totalDamage:240,hitCount:4,maxHit:75},
   {attack:"Dawncall",damageType:"spell",totalDamage:750,hitCount:6,maxHit:125}
  ]}/>);
  expect(html).toContain("Melee technique breakdown");
  expect(html).toContain("Slash");
  expect(html).toContain("100 avg");
  expect(html).toContain("Combat fingerprint");
  expect(html).toContain("Dawncall");
  expect(html).toContain("HEAVY HITTERS");
 });
});
