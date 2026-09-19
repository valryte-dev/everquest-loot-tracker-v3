import {describe,expect,it} from "vitest";
import type {MasterItem,Split} from "../../shared/contracts";
import {blockingCompoundComponents,canCreateCompoundProject,compoundOwners,compoundPotentialOwners,compoundSplitSaleAllocations,discordContributionSummary,eligibleProjectSplits,masterForItemName,mergeTemplateComponents,normalizeWorkspace,projectWarnings,receivedAfterAssignment,receivedAfterChoosingSplit,type CompoundProject,type CompoundTemplate} from "./model";

const items:MasterItem[]=[
 {id:101,name:"A Blue Throne",valuePp:12000,count30d:4,lastSeen:"today",manual:false,source:"inventory"},
 {id:202,name:"Cloak of Confusion",valuePp:50000,count30d:2,lastSeen:"today",manual:false,source:"inventory"},
];

describe("compound workspace model",()=>{
 it("allows unlinked recipe components while still rejecting blank components",()=>{
  expect(blockingCompoundComponents([{itemName:"A White Throne"}])).toEqual([]);
 expect(blockingCompoundComponents([{itemName:""}])).toHaveLength(1);
 });
 it("creates custom-named projects without requiring a master output ID",()=>{
  expect(canCreateCompoundProject("Plane of Mischief set",2,[{itemName:"A White Throne"}])).toBe(true);
  expect(canCreateCompoundProject("",2,[{itemName:"A White Throne"}])).toBe(false);
  expect(canCreateCompoundProject("Project",0,[{itemName:"A White Throne"}])).toBe(false);
 });
 it("matches an exactly typed master item name without requiring a result click",()=>{
  expect(masterForItemName("  Cloak of Confusion  ",items)?.id).toBe(202);
 });
 it("offers only unassigned split items that belong to the current recipe",()=>{
  const project:CompoundProject={id:"p",itemId:202,name:"Cloak of Confusion",note:"",status:"building",templates:[],components:[
   {id:"blue",itemId:101,itemName:"A Blue Throne",required:1,received:0,valuePp:12000,source:"personal",sourceRef:null,contributors:[],note:""},
   {id:"white",itemId:null,itemName:"A White Throne",required:1,received:1,valuePp:0,source:"split",sourceRef:"white-1",contributors:["Wes"],note:""},
  ]};
  const split=(key:string,itemName:string):Split=>({key,itemName,addedAt:"2026-09-13",attendees:["Wes"]});
  const splits=[split("blue-1","A Blue Throne"),split("white-1","A White Throne"),split("red-1","A Red Crown")];
  expect(eligibleProjectSplits(project,splits,items).map(row=>row.key)).toEqual(["blue-1"]);
  expect(eligibleProjectSplits(project,splits,items,"white-1").map(row=>row.key)).toEqual(["blue-1","white-1"]);
  expect(eligibleProjectSplits(project,splits,items,null,project.components[0]).map(row=>row.key)).toEqual(["blue-1"]);
  expect(eligibleProjectSplits(project,splits,items,"white-1",project.components[1]).map(row=>row.key)).toEqual(["white-1"]);
 });
 it("records one received component when an existing split is selected",()=>{
  expect(receivedAfterAssignment(2,0)).toBe(1);
  expect(receivedAfterChoosingSplit(1,0)).toBe(1);
  expect(receivedAfterChoosingSplit(2,0)).toBe(1);
  expect(receivedAfterChoosingSplit(2,2)).toBe(2);
  const restored=normalizeWorkspace({projects:[{id:"p",name:"Set",components:[{itemName:"A Blue Throne",required:2,received:0,source:"split",sourceRef:"loot:551",contributors:["Wes"]}]}]},items);
  expect(restored.projects[0].components).toHaveLength(2);
  expect(restored.projects[0].components[0].received).toBe(1);
  expect(restored.projects[0].components[0]).toMatchObject({required:1,sourceRef:"loot:551",contributors:["Wes"]});
  expect(restored.projects[0].components[1]).toMatchObject({required:1,received:0,source:"personal",sourceRef:null,contributors:[]});
  const assigned=normalizeWorkspace({projects:[{id:"p",name:"Set",components:[{itemName:"A Blue Throne",required:2,received:0,source:"personal",contributors:["Wes"]}]}]},items);
  expect(assigned.projects[0].components[0].received).toBe(1);
 });
 it("preserves rich V2 template metadata while normalizing names",()=>{
  const workspace=normalizeWorkspace({projects:[],templates:[{id:"saved",name:"Cloak of Confusion",components:[{name:"A Blue Throne",itemId:101,required:2,value:12000}]}]},items);
  expect(workspace.templates[0]).toMatchObject({id:"saved",name:"Cloak of Confusion",itemId:202});
  expect(workspace.templates[0].components[0]).toEqual({itemId:101,itemName:"A Blue Throne",required:2,valuePp:12000});
 });

 it("keeps duplicate required items as separately attributable unit rows",()=>{
  const templates:CompoundTemplate[]=[
   {id:"one",name:"One",itemId:null,components:[{itemId:101,itemName:"A Blue Throne",required:1,valuePp:12000}]},
   {id:"two",name:"Two",itemId:null,components:[{itemId:101,itemName:"A Blue Throne",required:2,valuePp:12000}]},
  ];
  const merged=mergeTemplateComponents(templates,items);
  expect(merged).toHaveLength(3);
  expect(merged.every(part=>part.itemId===101&&part.itemName==="A Blue Throne"&&part.required===1&&part.valuePp===12000)).toBe(true);
  expect(new Set(merged.map(part=>part.id)).size).toBe(3);
 });

 it("calculates alias-aware ownership and actionable warnings per project",()=>{
  const project:CompoundProject={id:"p",itemId:202,name:"Cloak of Confusion",note:"",status:"building",templates:[],components:[{id:"c",itemId:101,itemName:"A Blue Throne",required:2,received:1,valuePp:12000,source:"shared",sourceRef:null,contributors:["Youngman","Vinkledoo"],note:""}]};
  const owners=compoundOwners(project,[{alias:"Youngman",canonical:"Wes"},{alias:"Vinkledoo",canonical:"Wes"}]);
  expect(owners).toMatchObject([{name:"Wes",valuePp:12000,percent:100}]);
  expect(compoundPotentialOwners(project,[{alias:"Youngman",canonical:"Wes"},{alias:"Vinkledoo",canonical:"Wes"}])).toMatchObject([{name:"Wes",valuePp:24000,percent:100}]);
 expect(projectWarnings(project)).toContain("A Blue Throne: 1 still needed");
 });
 it("allocates a compound sale proportionally to received source splits",()=>{
  const project:CompoundProject={id:"p",itemId:202,name:"Set",note:"",status:"ready",templates:[],components:[
   {id:"a",itemId:101,itemName:"Split throne",required:1,received:1,valuePp:25000,source:"split",sourceRef:"loot:1",contributors:["A"],note:""},
   {id:"b",itemId:101,itemName:"Personal crown",required:1,received:1,valuePp:15000,source:"personal",sourceRef:null,contributors:["B"],note:""},
  ]};
  expect(compoundSplitSaleAllocations(project,80000)).toEqual([{key:"loot:1",itemName:"Split throne",valuePp:50000}]);
 });

 it("creates compact Discord markdown grouped by canonical person with item prices",()=>{
  const project:CompoundProject={id:"p",itemId:202,name:"Cloak of Confusion",note:"",status:"building",templates:[],components:[{id:"c",itemId:101,itemName:"A Blue Throne",required:1,received:1,valuePp:12000,source:"shared",sourceRef:null,contributors:["Youngman","Vinkledoo"],note:""}]};
  const markdown=discordContributionSummary(project,[{alias:"Youngman",canonical:"Wes"},{alias:"Vinkledoo",canonical:"Wes"}]);
  expect(markdown).toContain("**Cloak of Confusion — Contributions**");
  expect(markdown).toContain("**Wes** — 12,000 pp credit");
  expect(markdown).toContain("- A Blue Throne ×1 @ 12,000 pp each — 12,000 pp credit");
  expect(markdown.match(/\*\*Wes\*\*/g)).toHaveLength(1);
 });
});
