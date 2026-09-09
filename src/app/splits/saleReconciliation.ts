import type {Split} from "../../shared/contracts";

export type SaleMatchStatus="exact"|"likely"|"ambiguous"|"unmatched";
export interface ParsedSaleLine{id:number;lineNumber:number;raw:string;itemText:string;pricePp:number}
export interface SaleMatchCandidate{splitKey:string;itemName:string;holderName:string;score:number}
export interface SaleReconciliationRow extends ParsedSaleLine{matchedKey:string;status:SaleMatchStatus;confidence:number;candidates:SaleMatchCandidate[]}

const singular=(word:string)=>word.length>4&&word.endsWith("s")&&!word.endsWith("ss")?word.slice(0,-1):word;
export function normalizeSaleItem(value:string){return value.toLowerCase().replace(/^spell\s*:\s*/,"").replace(/[’'`]/g,"").replace(/[^a-z0-9]+/g," ").trim().split(/\s+/).filter(Boolean).map(singular).join(" ")}
const compact=(value:string)=>normalizeSaleItem(value).replace(/\s/g,"");
const acronym=(value:string)=>normalizeSaleItem(value).split(" ").filter(Boolean).map(word=>word[0]).join("");

function editDistance(a:string,b:string){const previous=Array.from({length:b.length+1},(_,index)=>index);for(let i=1;i<=a.length;i++){let diagonal=previous[0];previous[0]=i;for(let j=1;j<=b.length;j++){const above=previous[j],left=previous[j-1],cost=a[i-1]===b[j-1]?0:1;previous[j]=Math.min(above+1,left+1,diagonal+cost);diagonal=above}}return previous[b.length]}
function wordSimilarity(a:string,b:string){if(a===b)return 1;if(a.startsWith(b)||b.startsWith(a))return Math.min(a.length,b.length)/Math.max(a.length,b.length)*.7+.3;return 1-editDistance(a,b)/Math.max(a.length,b.length,1)}

export function scoreSaleItem(input:string,candidate:string){
 const query=normalizeSaleItem(input),target=normalizeSaleItem(candidate);if(!query||!target)return 0;
 if(query===target)return 1;
 if(compact(query)===compact(target))return .99;
 if(query.length<=6&&query.replace(/\s/g,"")===acronym(target))return .96;
 const queryWords=query.split(" "),targetWords=target.split(" ");
 if(target.includes(query)||query.includes(target))return Math.min(.95,.86+.09*Math.min(query.length,target.length)/Math.max(query.length,target.length));
 const directional=queryWords.reduce((sum,word)=>sum+Math.max(...targetWords.map(other=>wordSimilarity(word,other))),0)/queryWords.length;
 const coverage=Math.min(queryWords.length,targetWords.length)/Math.max(queryWords.length,targetWords.length);
 return Math.max(0,Math.min(.95,directional*(.78+.22*coverage)));
}

export function parseSoldList(text:string):ParsedSaleLine[]{
 const parsed=text.slice(0,50000).split(/\r?\n/).flatMap((raw,index)=>{const trimmed=raw.trim();if(!trimmed)return[];const match=trimmed.match(/^([\d,]+(?:\.\d+)?)\s*(?:[-\u2013\u2014:]|\s)\s*(.+?)\s*$/);if(!match)return[];const price=Math.round(Number(match[1].replace(/,/g,""))),itemText=match[2].trim();if(!Number.isFinite(price)||price<0||price>1_000_000_000||!itemText||itemText.length>200)return[];return[{id:index+1,lineNumber:index+1,raw,itemText,pricePp:price}]});
 return parsed.slice(0,250);
}
export function reconcileSoldList(text:string,splits:Split[]):SaleReconciliationRow[]{
 const used=new Set<string>();
 return parseSoldList(text).map(line=>{
  const candidates=splits.filter(split=>!used.has(split.key)).map(split=>({splitKey:split.key,itemName:split.itemName,holderName:split.looterName||"Unknown",score:scoreSaleItem(line.itemText,split.itemName)})).filter(candidate=>candidate.score>=.34).sort((a,b)=>b.score-a.score||a.itemName.localeCompare(b.itemName)||a.splitKey.localeCompare(b.splitKey,undefined,{numeric:true}));
  const best=candidates[0],nextDifferent=candidates.find(candidate=>best&&normalizeSaleItem(candidate.itemName)!==normalizeSaleItem(best.itemName));
  const decisive=!!best&&(best.score>=.96||(best.score>=.76&&(!nextDifferent||best.score-nextDifferent.score>=.1)));
  const matchedKey=decisive?best.splitKey:"";if(matchedKey)used.add(matchedKey);
  return{...line,matchedKey,status:matchedKey?(best.score>=.96?"exact":"likely"):(best?"ambiguous":"unmatched"),confidence:best?.score||0,candidates:candidates.slice(0,6)};
 });
}
