import type {Alias,History,Split} from "../../shared/contracts";

export type ContributionStatus="held"|"pending"|"paid"|"consumed";
export interface PayoutContribution{key:string;itemName:string;status:ContributionStatus;totalValuePp:number;shareValuePp:number;participantCount:number;mobName?:string;holderName?:string;note?:string;happenedAt:string;paidAt?:string}
export interface PersonPayoutSummary{name:string;heldSharePp:number;pendingSharePp:number;paidSharePp:number;consumedSharePp:number;totalTrackedPp:number;heldItems:number;pendingItems:number;paidItems:number;consumedItems:number;contributions:PayoutContribution[]}
export interface SplitPayoutSummary{people:PersonPayoutSummary[];heldValuePp:number;pendingValuePp:number;paidValuePp:number;consumedValuePp:number;heldCount:number;pendingCount:number;paidCount:number;consumedCount:number}

export function buildSplitPayoutSummary(splits:Split[],history:History[],aliases:Alias[]):SplitPayoutSummary{
 const aliasMap=new Map(aliases.map(alias=>[alias.alias.trim().toLowerCase(),alias.canonical.trim()]));
 const canonical=(name:string)=>aliasMap.get(name.trim().toLowerCase())||name.trim();
 const people=new Map<string,PersonPayoutSummary>();
 const getPerson=(name:string)=>{const display=canonical(name),key=display.toLowerCase();let person=people.get(key);if(!person){person={name:display,heldSharePp:0,pendingSharePp:0,paidSharePp:0,consumedSharePp:0,totalTrackedPp:0,heldItems:0,pendingItems:0,paidItems:0,consumedItems:0,contributions:[]};people.set(key,person)}return person};
 const participants=(names:string[])=>[...new Map(names.map(name=>[canonical(name).toLowerCase(),canonical(name)])).values()].filter(Boolean);
 const record=(name:string,row:Split|History,status:ContributionStatus,value:number,date:string,key:string,participantCount:number,paidAt?:string)=>{
  const person=getPerson(name),shareValuePp=Math.floor(value/Math.max(1,participantCount));
  const contribution:PayoutContribution={key,itemName:row.itemName,status,totalValuePp:value,shareValuePp,participantCount,mobName:row.mobName,holderName:row.looterName,happenedAt:date,paidAt};
  if("note" in row)contribution.note=row.note;
  person.contributions.push(contribution);
  if(status==="held"){person.heldSharePp+=shareValuePp;person.heldItems+=1}else if(status==="pending"){person.pendingSharePp+=shareValuePp;person.pendingItems+=1}else if(status==="paid"){person.paidSharePp+=shareValuePp;person.paidItems+=1}else{person.consumedSharePp+=shareValuePp;person.consumedItems+=1}
  person.totalTrackedPp+=shareValuePp;
 };
 splits.forEach(row=>{const names=participants(row.attendees);names.forEach(name=>record(name,row,"held",row.payoutValuePp??row.marketValuePp??0,row.addedAt,String(row.key),names.length))});
 history.forEach(row=>{
  const names=participants(row.attendees);
  const paidByCanonical=new Map((row.payouts||[]).map(payout=>[canonical(payout.name).toLowerCase(),payout.paidAt]));
  names.forEach(name=>{const paidAt=paidByCanonical.get(name.toLowerCase());const status:ContributionStatus=row.disposition==="consumed"?"consumed":paidAt?"paid":"pending";record(name,row,status,row.valuePp,row.completedAt,String(row.id),names.length,paidAt)});
 });
 const result=[...people.values()];result.forEach(person=>person.contributions.sort((a,b)=>(b.paidAt||b.happenedAt).localeCompare(a.paidAt||a.happenedAt)||a.itemName.localeCompare(b.itemName)));result.sort((a,b)=>b.pendingSharePp-a.pendingSharePp||b.totalTrackedPp-a.totalTrackedPp||a.name.localeCompare(b.name));
 const pendingRows=history.filter(row=>row.disposition==="sold"&&row.payoutStatus!=="completed"),paidRows=history.filter(row=>row.disposition==="sold"&&row.payoutStatus==="completed"),consumed=history.filter(row=>row.disposition==="consumed");
 return {people:result,heldValuePp:splits.reduce((sum,row)=>sum+(row.payoutValuePp??row.marketValuePp??0),0),pendingValuePp:result.reduce((sum,person)=>sum+person.pendingSharePp,0),paidValuePp:paidRows.reduce((sum,row)=>sum+row.valuePp,0),consumedValuePp:consumed.reduce((sum,row)=>sum+row.valuePp,0),heldCount:splits.length,pendingCount:pendingRows.length,paidCount:paidRows.length,consumedCount:consumed.length};
}
