import type {Alias,MasterItem} from "../../shared/contracts";

export type CompoundSource="personal"|"split"|"shared";

export interface CompoundComponent {
 id:string;
 itemId:number|null;
 itemName:string;
 required:number;
 received:number;
 valuePp:number;
 source:CompoundSource;
 sourceRef:string|null;
 contributors:string[];
 note:string;
}

export interface CompoundTemplateComponent {
 itemId:number|null;
 itemName:string;
 required:number;
 valuePp:number;
}

export interface CompoundTemplate {
 id:string;
 name:string;
 itemId:number|null;
 builtIn?:boolean;
 components:CompoundTemplateComponent[];
}

export interface CompoundProject {
 id:string;
 itemId:number|null;
 name:string;
 note:string;
 status:"building"|"ready"|"hold";
 templates:string[];
 components:CompoundComponent[];
}

export interface CompoundWorkspaceModel {
 projects:CompoundProject[];
 templates:CompoundTemplate[];
 activeId:string|null;
}

export interface CompoundOwner {
 name:string;
 valuePp:number;
 percent:number;
 parts:{itemName:string;valuePp:number}[];
}

export const newId=()=>crypto.randomUUID();
const text=(value:unknown,fallback="")=>typeof value==="string"?value:fallback;
const number=(value:unknown,fallback=0)=>Number.isFinite(Number(value))?Number(value):fallback;
const names=(value:unknown)=>Array.isArray(value)?value.map(item=>text(item).trim()).filter(Boolean):[];

const masterFor=(itemName:string,items:MasterItem[])=>items.find(item=>item.name.localeCompare(itemName,undefined,{sensitivity:"accent"})===0);

export function normalizeComponent(raw:any,items:MasterItem[]):CompoundComponent {
 const itemName=text(raw?.itemName,text(raw?.name)).trim();
 const master=masterFor(itemName,items);
 const source=["split","shared"].includes(raw?.source)?raw.source:"personal";
 return {
  id:text(raw?.id)||newId(),
  itemId:number(raw?.itemId,master?.id||0)||null,
  itemName:master?.name||itemName,
  required:Math.max(1,number(raw?.required,1)),
  received:Math.max(0,number(raw?.received,0)),
  valuePp:Math.max(0,number(raw?.valuePp,number(raw?.value,master?.valuePp||0))),
  source,
  sourceRef:text(raw?.sourceRef)||null,
  contributors:names(raw?.contributors??raw?.owners),
  note:text(raw?.note),
 };
}

export function normalizeTemplate(raw:any,items:MasterItem[]):CompoundTemplate {
 const name=text(raw?.name).trim();
 const output=masterFor(name,items);
 const components=(Array.isArray(raw?.components)?raw.components:[]).map((part:any)=>{
  const itemName=(typeof part==="string"?part:text(part?.itemName,text(part?.name))).trim();
  const master=masterFor(itemName,items);
  return {itemId:number(part?.itemId,master?.id||0)||null,itemName:master?.name||itemName,required:Math.max(1,number(part?.required,1)),valuePp:Math.max(0,number(part?.valuePp,number(part?.value,master?.valuePp||0)))};
 }).filter((part:CompoundTemplateComponent)=>part.itemName);
 return {id:text(raw?.id)||newId(),name,itemId:number(raw?.itemId,output?.id||0)||null,builtIn:Boolean(raw?.builtIn),components};
}

export function normalizeWorkspace(raw:any,items:MasterItem[]):CompoundWorkspaceModel {
 const templates:CompoundTemplate[]=(Array.isArray(raw?.templates)?raw.templates:[]).map((template:any)=>normalizeTemplate(template,items)).filter((template:CompoundTemplate)=>template.name);
 const projects:CompoundProject[]=(Array.isArray(raw?.projects)?raw.projects:[]).map((rawProject:any):CompoundProject=>{
  const name=text(rawProject?.name).trim();
  const output=masterFor(name,items);
  return {
   id:text(rawProject?.id)||newId(),itemId:number(rawProject?.itemId,output?.id||0)||null,name:output?.name||name,
   note:text(rawProject?.note),status:["ready","hold"].includes(rawProject?.status)?rawProject.status:"building",
   templates:names(rawProject?.templates),components:(Array.isArray(rawProject?.components)?rawProject.components:[]).map((part:any)=>normalizeComponent(part,items)).filter((part:CompoundComponent)=>part.itemName),
  };
 });
 const activeId=text(raw?.activeId)||projects[0]?.id||null;
 return {projects,templates,activeId:projects.some(project=>project.id===activeId)?activeId:projects[0]?.id||null};
}

export function mergeTemplateComponents(templates:CompoundTemplate[],items:MasterItem[]):CompoundComponent[] {
 const merged=new Map<string,CompoundComponent>();
 for(const template of templates)for(const source of template.components){
  const master=masterFor(source.itemName,items);
  const itemName=master?.name||source.itemName;
  const key=itemName.toLocaleLowerCase();
  const existing=merged.get(key);
  if(existing){existing.required+=source.required;continue}
  merged.set(key,{id:newId(),itemId:source.itemId||master?.id||null,itemName,required:source.required,received:0,valuePp:source.valuePp||master?.valuePp||0,source:"personal",sourceRef:null,contributors:[],note:""});
 }
 return [...merged.values()];
}

export function projectProgress(project:CompoundProject){
 const required=project.components.reduce((sum,part)=>sum+part.required,0);
 const received=project.components.reduce((sum,part)=>sum+Math.min(part.received,part.required),0);
 return {required,received,percent:required?Math.round(received/required*100):0};
}

export const projectValue=(project:CompoundProject)=>project.components.reduce((sum,part)=>sum+part.required*part.valuePp,0);
export const componentCredit=(part:CompoundComponent)=>part.received*part.valuePp;

export function compoundOwners(project:CompoundProject,aliases:Alias[]):CompoundOwner[] {
 const aliasMap=new Map(aliases.map(alias=>[alias.alias.toLocaleLowerCase(),alias.canonical]));
 const values=new Map<string,{name:string;valuePp:number;parts:{itemName:string;valuePp:number}[]}>();
 for(const part of project.components){
  const canonical=[...new Map(part.contributors.map(name=>{const resolved=aliasMap.get(name.toLocaleLowerCase())||name;return[resolved.toLocaleLowerCase(),resolved]})).values()];
  if(!canonical.length)continue;
  const each=componentCredit(part)/canonical.length;
  for(const name of canonical){const key=name.toLocaleLowerCase(),entry=values.get(key)||{name,valuePp:0,parts:[]};entry.valuePp+=each;entry.parts.push({itemName:part.itemName,valuePp:each});values.set(key,entry)}
 }
 const total=[...values.values()].reduce((sum,entry)=>sum+entry.valuePp,0);
 return [...values.values()].map(entry=>({...entry,percent:total?entry.valuePp/total*100:0})).sort((a,b)=>b.valuePp-a.valuePp||a.name.localeCompare(b.name));
}

export function projectWarnings(project:CompoundProject):string[] {
 const warnings:string[]=[];
 for(const part of project.components){
  if(part.received<part.required)warnings.push(`${part.itemName}: ${part.required-part.received} still needed`);
  if(part.received>0&&!part.contributors.length)warnings.push(`${part.itemName}: no contributor assigned`);
  if(part.required>0&&!part.valuePp)warnings.push(`${part.itemName}: no stored value`);
  if(!part.itemId)warnings.push(`${part.itemName}: master item not linked`);
  if(part.received>part.required)warnings.push(`${part.itemName}: ${part.received-part.required} over recipe quantity`);
 }
 if(!project.itemId)warnings.unshift(`${project.name}: compound output is not linked to a master item`);
 return warnings;
}

const discordMoney=(value:number)=>`${Math.round(value).toLocaleString("en-US")} pp`;

export function discordContributionSummary(project:CompoundProject,aliases:Alias[]):string {
 const aliasMap=new Map(aliases.map(alias=>[alias.alias.toLocaleLowerCase(),alias.canonical]));
 const people=new Map<string,{name:string;credit:number;items:string[]}>();
 for(const part of project.components){
  if(part.received<=0||!part.contributors.length)continue;
  const contributors=[...new Map(part.contributors.map(name=>{const canonical=aliasMap.get(name.toLocaleLowerCase())||name;return[canonical.toLocaleLowerCase(),canonical]})).values()];
  const credit=componentCredit(part)/contributors.length;
  for(const name of contributors){
   const key=name.toLocaleLowerCase(),person=people.get(key)||{name,credit:0,items:[]};
   person.credit+=credit;
   const shared=contributors.length>1?" · shared":"";
   person.items.push(`- ${part.itemName} ×${part.received} @ ${discordMoney(part.valuePp)} each — ${discordMoney(credit)} credit${shared}`);
   people.set(key,person);
  }
 }
 const progress=projectProgress(project);
 const lines=[`**${project.name} — Contributions**`,`Progress: **${progress.received} / ${progress.required}** · Estimated recipe value: **${discordMoney(projectValue(project))}**`];
 for(const person of [...people.values()].sort((a,b)=>a.name.localeCompare(b.name))){
  lines.push("",`**${person.name}** — ${discordMoney(person.credit)} credit`,...person.items);
 }
 if(!people.size)lines.push("","_No received components have assigned contributors yet._");
 const result=lines.join("\n");
 return result.length<=1900?result:`${result.slice(0,1860).trimEnd()}\n\n_Additional contributions omitted._`;
}
