import {describe,expect,it} from "vitest";
import {ARMOR_ARCHETYPES,ARMOR_SLOTS,VELIOUS_ARMOR_GEMS,gemFor,isVeliousArmorGem} from "./catalog";

describe("Velious armor gem catalog",()=>{
 it("stores all 18 special gems from the local reference table",()=>expect(VELIOUS_ARMOR_GEMS).toHaveLength(18));
 it("defines one gem for every armor archetype and slot",()=>{for(const archetype of ARMOR_ARCHETYPES)for(const slot of ARMOR_SLOTS)expect(gemFor(archetype,slot),`${archetype} ${slot}`).toBeDefined()});
 it("reuses gems that serve more than one armor archetype",()=>expect(VELIOUS_ARMOR_GEMS.find(gem=>gem.name==="Crushed Topaz")?.usages).toHaveLength(2));
 it("matches gem names exactly without being case-sensitive",()=>{expect(isVeliousArmorGem("crushed topaz")).toBe(true);expect(isVeliousArmorGem("Crushed Topaz Ring")).toBe(false)});
});
