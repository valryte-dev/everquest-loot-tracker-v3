import type {PortraitCharacterPlacement,PortraitProject} from "../../shared/contracts";

export const DEFAULT_PORTRAIT_PROMPT="A polished hand-drawn fantasy family portrait. Preserve every character's race, face, armor colors, exact weapons, shield, handedness, relative height, and formation from the supplied EverQuest model reference. Use clean expressive linework, restrained painterly shading, and warm tavern lighting. Do not replace, invent, crop, or redesign equipment.";

export function projectId(name:string){return (name.trim().toLocaleLowerCase().replace(/[^a-z0-9]+/g,"-").replace(/^-|-$/g,"").slice(0,60)||`portrait-${Date.now()}`)}

export function newPortraitProject(name="EQ Family Portrait"):PortraitProject{return{version:1,id:projectId(name),name,updatedAt:new Date(0).toISOString(),background:"tavern",aspectRatio:"16:9",characters:[]}}

export function formation(names:string[],preset:"line"|"family"|"raid"="family"):PortraitCharacterPlacement[]{
 const count=Math.max(1,names.length);
 return names.map((name,index)=>{
  const t=count===1?.5:index/(count-1);
  if(preset==="line")return{name,x:.15+t*.7,y:.52,scale:.88,layer:index};
  if(preset==="raid"){const row=index%2,column=Math.floor(index/2),columns=Math.ceil(count/2);return{name,x:.15+(columns===1?.5:column/(columns-1))*.7,y:row?0.66:0.42,scale:row?.78:.9,layer:row?index:count+index}}
  const center=(count-1)/2,distance=Math.abs(index-center);return{name,x:.14+t*.72,y:.52+distance*.025,scale:Math.max(.72,1-distance*.06),layer:Math.round(100-distance*10)};
 });
}

export function downloadDataUrl(dataUrl:string,fileName:string){const link=document.createElement("a");link.href=dataUrl;link.download=fileName;link.click()}
