import {describe,expect,it} from "vitest";
import {formation,newPortraitProject,projectId} from "./model";
describe("portrait studio model",()=>{
 it("creates portable versioned projects",()=>{expect(newPortraitProject("Family")).toMatchObject({version:1,id:"family",name:"Family",characters:[]})});
  it("keeps formations inside the wide-model safe area",()=>{for(const preset of ["family","line","raid"] as const)for(const row of formation(["A","B","C","D","E"],preset)){expect(row.x).toBeGreaterThanOrEqual(.14);expect(row.x).toBeLessThanOrEqual(.86);expect(row.scale).toBeGreaterThan(0)}});
 it("creates safe project ids",()=>expect(projectId("  Valryte & Family!  ")).toBe("valryte-family"));
});
