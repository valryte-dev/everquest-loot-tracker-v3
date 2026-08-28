import {describe,expect,it} from "vitest";
import type {MasterItem} from "../../shared/contracts";
import {compoundOwners,discordContributionSummary,mergeTemplateComponents,normalizeWorkspace,projectWarnings,type CompoundProject,type CompoundTemplate} from "./model";

const items:MasterItem[]=[
 {id:101,name:"A Blue Throne",valuePp:12000,count30d:4,lastSeen:"today",manual:false,source:"inventory"},
 {id:202,name:"Cloak of Confusion",valuePp:50000,count30d:2,lastSeen:"today",manual:false,source:"inventory"},
];

describe("compound workspace model",()=>{
 it("preserves rich V2 template metadata while normalizing names",()=>{
  const workspace=normalizeWorkspace({projects:[],templates:[{id:"saved",name:"Cloak of Confusion",components:[{name:"A Blue Throne",itemId:101,required:2,value:12000}]}]},items);
  expect(workspace.templates[0]).toMatchObject({id:"saved",name:"Cloak of Confusion",itemId:202});
  expect(workspace.templates[0].components[0]).toEqual({itemId:101,itemName:"A Blue Throne",required:2,valuePp:12000});
 });

 it("merges duplicate components and adds their required quantities",()=>{
  const templates:CompoundTemplate[]=[
   {id:"one",name:"One",itemId:null,components:[{itemId:101,itemName:"A Blue Throne",required:1,valuePp:12000}]},
   {id:"two",name:"Two",itemId:null,components:[{itemId:101,itemName:"A Blue Throne",required:2,valuePp:12000}]},
  ];
  const merged=mergeTemplateComponents(templates,items);
  expect(merged).toHaveLength(1);
  expect(merged[0]).toMatchObject({itemId:101,itemName:"A Blue Throne",required:3,valuePp:12000});
 });

 it("calculates alias-aware ownership and actionable warnings per project",()=>{
  const project:CompoundProject={id:"p",itemId:202,name:"Cloak of Confusion",note:"",status:"building",templates:[],components:[{id:"c",itemId:101,itemName:"A Blue Throne",required:2,received:1,valuePp:12000,source:"shared",sourceRef:null,contributors:["Youngman","Vinkledoo"],note:""}]};
  const owners=compoundOwners(project,[{alias:"Youngman",canonical:"Wes"},{alias:"Vinkledoo",canonical:"Wes"}]);
  expect(owners).toMatchObject([{name:"Wes",valuePp:12000,percent:100}]);
 expect(projectWarnings(project)).toContain("A Blue Throne: 1 still needed");
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
