export interface MapCatalogZone { shortName:string; label:string; aliases:string[]; files:string[] }
export interface MapLine { x1:number; y1:number; z1:number; x2:number; y2:number; z2:number; color:string; layer:number }
export interface MapLabel { x:number; y:number; z:number; color:string; size:number; text:string; layer:number }
export interface ParsedMap { lines:MapLine[]; labels:MapLabel[] }

const finite=(value:string)=>{const parsed=Number(value);return Number.isFinite(parsed)?parsed:undefined};
const color=(parts:string[],offset:number)=>{const values=parts.slice(offset,offset+3).map(Number);return values.length===3&&values.every(value=>Number.isFinite(value)&&value>=0&&value<=255)?`rgb(${values[0]},${values[1]},${values[2]})`:"rgb(145,160,151)"};
export const layerFromFile=(file:string)=>Number(file.match(/_(\d+)\.txt$/i)?.[1]||0);
export function parseMapText(text:string,layer=0):ParsedMap{const lines:MapLine[]=[],labels:MapLabel[]=[];for(const raw of text.split(/\r?\n/)){const trimmed=raw.trim();if(!trimmed)continue;const kind=trimmed[0]?.toUpperCase(),parts=trimmed.slice(1).trim().split(/\s*,\s*/);if(kind==="L"&&parts.length>=9){const values=parts.slice(0,6).map(finite);if(values.every(value=>value!==undefined))lines.push({x1:values[0]!,y1:values[1]!,z1:values[2]!,x2:values[3]!,y2:values[4]!,z2:values[5]!,color:color(parts,6),layer})}if(kind==="P"&&parts.length>=8){const values=parts.slice(0,3).map(finite),size=finite(parts[6]);if(values.every(value=>value!==undefined)&&size!==undefined)labels.push({x:values[0]!,y:values[1]!,z:values[2]!,color:color(parts,3),size,text:parts.slice(7).join(",").replaceAll("_"," "),layer})}}return{lines,labels}}
export const normalizeZone=(value:string)=>value.toLowerCase().replace(/^the\s+/,'').replace(/[^a-z0-9]+/g,' ').trim();
export function resolveZone(catalog:MapCatalogZone[],value:string){const wanted=normalizeZone(value);if(!wanted)return undefined;return catalog.find(zone=>[zone.shortName,zone.label,...zone.aliases].some(alias=>normalizeZone(alias)===wanted))}
export function mapPlayerPosition(northSouth:number,eastWest:number){return{x:-eastWest,y:-northSouth}}
export function mapHeadingRadians(headingDegrees:number){return(-headingDegrees*Math.PI)/180}
export function readableMapColor(value:string,minimum=120){const match=value.match(/rgb\((\d+),(\d+),(\d+)\)/);if(!match)return value;const channels=match.slice(1).map(Number),peak=Math.max(...channels);if(peak>=minimum)return value;if(peak===0)return`rgb(${minimum},${minimum+12},${minimum+6})`;const factor=minimum/peak;return`rgb(${channels.map(channel=>Math.min(255,Math.round(channel*factor))).join(",")})`}
