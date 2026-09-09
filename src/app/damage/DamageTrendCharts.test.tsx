import {describe,expect,it} from "vitest";
import {renderToStaticMarkup} from "react-dom/server";
import type {DamageEvent} from "../../shared/contracts";
import {DamageTrendCharts} from "./DamageTrendCharts";

const events:DamageEvent[]=[
 {id:1,happenedAt:"2026-09-08 10:00:01",attacker:"Valmezz",damageType:"melee",attack:"hit",damage:60},
 {id:2,happenedAt:"2026-09-08 10:00:02",attacker:"Treasure Chest",damageType:"melee",attack:"hit",damage:30},
];

describe("DamageTrendCharts",()=>{
 it("renders cumulative and rolling DPS lines with stable fighter colors",()=>{
  const html=renderToStaticMarkup(<DamageTrendCharts events={events} startedAt="2026-09-08 10:00:00" names={["Valmezz","Treasure Chest"]} durationSeconds={2}/>);
  expect(html).toContain("Cumulative damage");
  expect(html).toContain("Rolling DPS");
  expect(html).toContain("30-second window");
  expect(html).toContain("#fb56a3");
  expect(html).toContain("#a878fa");
  expect(html).toContain("Valmezz");
  expect(html).toContain("Treasure Chest");
 });
});