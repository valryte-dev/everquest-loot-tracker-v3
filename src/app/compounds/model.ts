import type {Alias,CompoundComponent,CompoundProject,CompoundSource,CompoundTemplate,CompoundTemplateComponent,CompoundWorkspace,MasterItem,Split} from "../../shared/contracts";
export type {CompoundComponent,CompoundProject,CompoundSource,CompoundTemplate,CompoundTemplateComponent,CompoundWorkspace};
export type CompoundWorkspaceModel=CompoundWorkspace;
export interface CompoundOwner {
 name:string;
 valuePp:number;
 percent:number;
 parts:{itemName:string;valuePp:number}[];
}
export interface CompoundSplitSaleAllocation { key:string; itemName:string; valuePp:number }

export const newId=()=>crypto.randomUUID();
const record=(value:unknown):Record<string,unknown>=>typeof value==="object"&&value!==null&&!Array.isArray(value)?value as Record<string,unknown>:{};
const text=(value:unknown,fallback="")=>typeof value==="string"?value:fallback;
const number=(value:unknown,fallback=0)=>Number.isFinite(Number(value))?Number(value):fallback;
const names=(value:unknown)=>Array.isArray(value)?value.map(item=>text(item).trim()).filter(Boolean):[];

export const masterForItemName=(itemName:string,items:MasterItem[])=>items.find(item=>item.name.localeCompare(itemName.trim(),undefined,{sensitivity:"accent"})===0);
const masterFor=masterForItemName;

export const blockingCompoundComponents=(components:Pick<CompoundComponent,"itemName">[])=>components.filter(part=>!part.itemName.trim());
export const canCreateCompoundProject=(name:string,templateCount:number,components:Pick<CompoundComponent,"itemName">[])=>
 Boolean(name.trim()&&templateCount>0&&!blockingCompoundComponents(components).length);

export const receivedAfterAssignment=(required:number,currentReceived:number)=>
 Math.min(Math.max(1,required),Math.max(1,currentReceived));

export const receivedAfterChoosingSplit=receivedAfterAssignment;

const normalizedItemName=(value:string)=>value.trim().toLocaleLowerCase();

export function eligibleProjectSplits(
 project:Pick<CompoundProject,"components">,
 splits:Split[],
 items:MasterItem[],
 currentSourceRef:string|null=null,
 targetComponent:Pick<CompoundComponent,"itemId"|"itemName">|null=null,
):Split[] {
 const eligibleComponents=targetComponent?[targetComponent]:project.components;
 const componentIds=new Set(eligibleComponents.map(part=>part.itemId).filter((id):id is number=>id!==null));
 const componentNames=new Set(eligibleComponents.map(part=>normalizedItemName(part.itemName)).filter(Boolean));
 const assigned=new Set(project.components
  .filter(part=>part.source==="split"&&part.sourceRef&&part.sourceRef!==currentSourceRef)
  .map(part=>part.sourceRef as string));
 return splits.filter(split=>{
  if(assigned.has(split.key))return false;
  const master=masterFor(split.itemName,items);
  return Boolean((master&&componentIds.has(master.id))||componentNames.has(normalizedItemName(split.itemName)));
 }).sort((a,b)=>a.itemName.localeCompare(b.itemName)||a.addedAt.localeCompare(b.addedAt));
}

export function normalizeComponent(raw:unknown,items:MasterItem[]):CompoundComponent {
 const value=record(raw);
 const itemName=text(value.itemName,text(value.name)).trim();
 const master=masterFor(itemName,items);
 const source:CompoundSource=value.source==="split"||value.source==="shared"?value.source:"personal";
 const sourceRef=text(value.sourceRef)||null;
 const required=Math.max(1,number(value.required,1));
 const rawReceived=Math.max(0,number(value.received,0));
 const contributors=names(value.contributors??value.owners);
 const assigned=Boolean(contributors.length||(source==="split"&&sourceRef));
 return {
  id:text(value.id)||newId(),
  itemId:number(value.itemId,master?.id||0)||null,
  itemName:master?.name||itemName,
  required,
  received:assigned?receivedAfterAssignment(required,rawReceived):rawReceived,
  valuePp:Math.max(0,number(value.valuePp,number(value.value,master?.valuePp||0))),
  source,
  sourceRef,
  contributors,
  note:text(value.note),
 };
}

export function expandCompoundComponent(component:CompoundComponent):CompoundComponent[] {
 const count=Math.max(1,component.required);
 const received=Math.min(count,Math.max(0,component.received));
 return Array.from({length:count},(_,index)=>({
  ...component,
  id:index===0?component.id:`${component.id}:unit:${index+1}`,
  required:1,
  received:index<received?1:0,
  source:index===0?component.source:"personal",
  sourceRef:index===0?component.sourceRef:null,
  contributors:index===0?component.contributors:[],
  note:index===0?component.note:"",
 }));
}

export function normalizeTemplate(raw:unknown,items:MasterItem[]):CompoundTemplate {
 const value=record(raw);
 const name=text(value.name).trim();
 const output=masterFor(name,items);
 const components:CompoundTemplateComponent[]=(Array.isArray(value.components)?value.components:[]).map(part=>{
  const component=record(part);
  const itemName=(typeof part==="string"?part:text(component.itemName,text(component.name))).trim();
  const master=masterFor(itemName,items);
  return {itemId:number(component.itemId,master?.id||0)||null,itemName:master?.name||itemName,required:Math.max(1,number(component.required,1)),valuePp:Math.max(0,number(component.valuePp,number(component.value,master?.valuePp||0)))};
 }).filter(part=>Boolean(part.itemName));
 return {id:text(value.id)||newId(),name,itemId:number(value.itemId,output?.id||0)||null,builtIn:Boolean(value.builtIn),components};
}

export function normalizeWorkspace(raw:unknown,items:MasterItem[]):CompoundWorkspaceModel {
 const value=record(raw);
 const templates:CompoundTemplate[]=(Array.isArray(value.templates)?value.templates:[]).map(template=>normalizeTemplate(template,items)).filter(template=>Boolean(template.name));
 const projects:CompoundProject[]=(Array.isArray(value.projects)?value.projects:[]).map((project):CompoundProject=>{
  const rawProject=record(project);
  const name=text(rawProject.name).trim();
  const output=masterFor(name,items);
  const status:CompoundProject["status"]=rawProject.status==="ready"||rawProject.status==="hold"||rawProject.status==="sold"?rawProject.status:"building";
  return {
   id:text(rawProject.id)||newId(),itemId:number(rawProject.itemId,output?.id||0)||null,name:output?.name||name,
   note:text(rawProject.note),status,
   soldAt:text(rawProject.soldAt)||undefined,
   saleValuePp:rawProject.saleValuePp===undefined?undefined:Math.max(0,number(rawProject.saleValuePp)),
   saleNote:text(rawProject.saleNote)||undefined,
   templates:names(rawProject.templates),components:(Array.isArray(rawProject.components)?rawProject.components:[]).flatMap(part=>expandCompoundComponent(normalizeComponent(part,items))).filter(part=>Boolean(part.itemName)),
  };
 });
 const activeId=text(value.activeId)||projects[0]?.id||null;
 return {projects,templates,activeId:projects.some(project=>project.id===activeId)?activeId:projects[0]?.id||null};
}
export function mergeTemplateComponents(templates:CompoundTemplate[],items:MasterItem[]):CompoundComponent[] {
 const components:CompoundComponent[]=[];
 for(const template of templates)for(const source of template.components){
  const master=masterFor(source.itemName,items);
  const itemName=master?.name||source.itemName;
  for(let index=0;index<Math.max(1,source.required);index++)components.push({id:newId(),itemId:source.itemId||master?.id||null,itemName,required:1,received:0,valuePp:source.valuePp||master?.valuePp||0,source:"personal",sourceRef:null,contributors:[],note:""});
 }
 return components;
}

export function projectProgress(project:CompoundProject){
 const required=project.components.reduce((sum,part)=>sum+part.required,0);
 const received=project.components.reduce((sum,part)=>sum+Math.min(part.received,part.required),0);
 return {required,received,percent:required?Math.round(received/required*100):0};
}

export const projectValue=(project:CompoundProject)=>project.components.reduce((sum,part)=>sum+part.required*part.valuePp,0);
export const componentCredit=(part:CompoundComponent)=>part.received*part.valuePp;

export function compoundSplitSaleAllocations(project:CompoundProject,saleValuePp:number):CompoundSplitSaleAllocation[] {
 const linked=new Map<string,{itemName:string;weight:number}>();
 for(const part of project.components){
  if(part.source!=="split"||!part.sourceRef||part.received<1)continue;
  const current=linked.get(part.sourceRef)||{itemName:part.itemName,weight:0};
  current.weight+=Math.max(0,part.valuePp);
  linked.set(part.sourceRef,current);
 }
 const projectWeight=project.components.reduce((sum,part)=>sum+Math.max(0,part.valuePp),0);
 const fallbackWeight=[...linked.values()].reduce((sum,row)=>sum+row.weight,0);
 return [...linked].map(([key,row])=>({
  key,
  itemName:row.itemName,
  valuePp:Math.max(0,Math.round(saleValuePp*(projectWeight>0?row.weight/projectWeight:1/Math.max(1,linked.size)))),
 })).filter(row=>fallbackWeight>0||linked.size>0);
}

function compoundOwnersUsing(project:CompoundProject,aliases:Alias[],creditFor:(part:CompoundComponent)=>number):CompoundOwner[] {
 const aliasMap=new Map(aliases.map(alias=>[alias.alias.toLocaleLowerCase(),alias.canonical]));
 const values=new Map<string,{name:string;valuePp:number;parts:{itemName:string;valuePp:number}[]}>();
 for(const part of project.components){
  const canonical=[...new Map(part.contributors.map(name=>{const resolved=aliasMap.get(name.toLocaleLowerCase())||name;return[resolved.toLocaleLowerCase(),resolved]})).values()];
  if(!canonical.length)continue;
  const each=creditFor(part)/canonical.length;
  for(const name of canonical){const key=name.toLocaleLowerCase(),entry=values.get(key)||{name,valuePp:0,parts:[]};entry.valuePp+=each;entry.parts.push({itemName:part.itemName,valuePp:each});values.set(key,entry)}
 }
 const total=[...values.values()].reduce((sum,entry)=>sum+entry.valuePp,0);
 return [...values.values()].map(entry=>({...entry,percent:total?entry.valuePp/total*100:0})).sort((a,b)=>b.valuePp-a.valuePp||a.name.localeCompare(b.name));
}

export const compoundOwners=(project:CompoundProject,aliases:Alias[])=>
 compoundOwnersUsing(project,aliases,componentCredit);

export const compoundPotentialOwners=(project:CompoundProject,aliases:Alias[])=>
 compoundOwnersUsing(project,aliases,part=>part.required*part.valuePp);

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
