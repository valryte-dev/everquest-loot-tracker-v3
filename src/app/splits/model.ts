import type {Alias,History,Split} from "../../shared/contracts";

export type ContributionStatus="held"|"pending"|"paid"|"consumed";
export interface PayoutContribution{key:string;itemName:string;status:ContributionStatus;totalValuePp:number;shareValuePp:number;participantCount:number;mobName?:string;holderName?:string;note?:string;happenedAt:string;paidAt?:string}
export interface PersonPayoutSummary{name:string;heldSharePp:number;pendingSharePp:number;paidSharePp:number;consumedSharePp:number;totalTrackedPp:number;heldItems:number;pendingItems:number;paidItems:number;consumedItems:number;contributions:PayoutContribution[]}
export interface SplitPayoutSummary{people:PersonPayoutSummary[];heldValuePp:number;pendingValuePp:number;paidValuePp:number;consumedValuePp:number;heldCount:number;pendingCount:number;paidCount:number;consumedCount:number}
export interface SplitPeopleGroups{current:string[];acrossSplits:string[];others:string[]}

export function groupSplitPeople(current:string[],splitPeople:string[],members:string[],query=""):SplitPeopleGroups{
 const unique=(names:string[])=>[...new Map(names.map(name=>name.trim()).filter(Boolean).map(name=>[name.toLowerCase(),name])).values()];
 const matches=(name:string)=>name.toLowerCase().includes(query.trim().toLowerCase());
 const sort=(names:string[])=>names.filter(matches).sort((a,b)=>a.localeCompare(b));
 const currentNames=unique(current),currentKeys=new Set(currentNames.map(name=>name.toLowerCase()));
 const acrossSplits=unique(splitPeople).filter(name=>!currentKeys.has(name.toLowerCase())),splitKeys=new Set(acrossSplits.map(name=>name.toLowerCase()));
 const others=unique(members).filter(name=>!currentKeys.has(name.toLowerCase())&&!splitKeys.has(name.toLowerCase()));
 return {current:sort(currentNames),acrossSplits:sort(acrossSplits),others:sort(others)};
}

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
const discordMoney=(value:number)=>`${Math.round(value).toLocaleString("en-US")} pp`;

const heldPeople=(summary:SplitPayoutSummary)=>summary.people
 .filter(person=>person.heldItems>0)
 .sort((a,b)=>b.heldSharePp-a.heldSharePp||a.name.localeCompare(b.name));

const heldContributionLines=(person:PersonPayoutSummary)=>person.contributions
 .filter(item=>item.status==="held")
 .sort((a,b)=>b.shareValuePp-a.shareValuePp||a.itemName.localeCompare(b.itemName))
 .map(item=>{
  const context=[item.holderName?`held by ${item.holderName}`:"",item.mobName?`dropped by ${item.mobName}`:""].filter(Boolean);
  return `- ${item.itemName} - **${discordMoney(item.shareValuePp)} share** (${discordMoney(item.totalValuePp)} / ${item.participantCount})${context.length?` - ${context.join("; ")}`:""}`;
 });

const boundDiscordMessage=(lines:string[])=>{
 const limit=1900,omission="\n\n_Additional payout details omitted._",message=lines.join("\n");
 if(message.length<=limit)return message;
 const kept:string[]=[];
 for(const line of lines){if([...kept,line].join("\n").length+omission.length>limit)break;kept.push(line)}
 return `${kept.join("\n")}${omission}`;
};

export function discordPotentialPayoutBriefSummary(summary:SplitPayoutSummary):string{
 const people=heldPeople(summary);
 if(!people.length)return "**Potential Split Payouts**\n_No unsold split loot is currently tracked._";
 return boundDiscordMessage(["**Potential Split Payouts**",...people.map(person=>`- **${person.name}:** ${discordMoney(person.heldSharePp)}`)]);
}

export function discordPlayerPotentialPayoutSummary(person:PersonPayoutSummary):string{
 return boundDiscordMessage([
  `**${person.name} - Potential Split Payout**`,
  `Total: **${discordMoney(person.heldSharePp)}** across **${person.heldItems} item${person.heldItems===1?"":"s"}**`,
  "*Unsold loot estimate - final payout may change when items sell.*",
  "",
  ...heldContributionLines(person),
 ]);
}

export function discordPotentialPayoutSummary(summary:SplitPayoutSummary):string{
 const people=heldPeople(summary);
 if(!people.length)return "**Potential Split Payouts**\n_No unsold split loot is currently tracked._";
 const estimatedShares=people.reduce((sum,person)=>sum+person.heldSharePp,0);
 const lines=[
  "**Potential Split Payouts**",
  "*Unsold loot estimates - final payouts may change when items sell.*",
  `Tracked loot: **${discordMoney(summary.heldValuePp)}** across **${summary.heldCount} item${summary.heldCount===1?"":"s"}**`,
  `Estimated player shares: **${discordMoney(estimatedShares)}**`,
 ];
 people.forEach(person=>{
  lines.push("",`**${person.name} - ${discordMoney(person.heldSharePp)} potential**`);
  lines.push(...heldContributionLines(person));
 });
 return boundDiscordMessage(lines);
}

export function discordSoldItemsSummary(history:History[],aliases:Alias[]):string{
 const aliasMap=new Map(aliases.map(alias=>[alias.alias.trim().toLowerCase(),alias.canonical.trim()]));
 const canonical=(name:string)=>aliasMap.get(name.trim().toLowerCase())||name.trim();
 const lines=history.filter(row=>row.disposition==="sold").map(row=>{
  const people=[...new Map(row.attendees.map(name=>canonical(name)).filter(Boolean).map(name=>[name.toLowerCase(),name])).values()];
  const splitValue=Math.floor(row.valuePp/Math.max(1,people.length));
  return `- **${row.itemName}** - Sale price: **${discordMoney(row.valuePp)}** - Split names: ${people.join(", ")||"None"} - Split value: **${discordMoney(splitValue)} each**`;
 });
 return lines.length?boundDiscordMessage(lines):"_No sold split items to share._";
}

const discordDetailCell=(value:string|undefined)=>value?.replace(/[\r\n]+/g," ").trim()||"-";

function soldItemsDiscordMessages(title:string,rows:{valuePp:number}[],rowLines:string[],emptyText="No sold split items to share.",valueLabel="total sale value"):string[]{
 if(!rows.length)return ["**"+title+"**\n_"+emptyText+"_"];
 const saleTotal=rows.reduce((sum,row)=>sum+row.valuePp,0);
 const bodyLimit=1750,segments:string[]=[];
 rowLines.forEach(rowLine=>{
  let remaining=rowLine;
  while(remaining.length>bodyLimit){
   const naturalBreak=remaining.lastIndexOf(" - ",bodyLimit),breakAt=naturalBreak>0?naturalBreak:bodyLimit;
   segments.push(remaining.slice(0,breakAt));
   remaining=remaining.slice(breakAt).replace(/^ - /,"- ");
  }
  if(remaining)segments.push(remaining);
 });
 const messages:string[]=[];
 segments.forEach(segment=>{
  const index=messages.length-1,current=messages[index];
  if(current&&current.length+segment.length+1<=bodyLimit)messages[index]=current+"\n"+segment;
  else messages.push(segment);
 });
 return messages.map((body,index)=>[
  "**"+title+"**",
  "_Message "+(index+1)+" of "+messages.length+"_",
  ...(index===0?["**"+rows.length+" items | "+discordMoney(saleTotal)+" "+valueLabel+"**"]:[]),
  body,
 ].join("\n"));
}

function soldItemCanonicalizer(aliases:Alias[]){
 const aliasMap=new Map(aliases.map(alias=>[alias.alias.trim().toLowerCase(),alias.canonical.trim()]));
 return (name:string)=>aliasMap.get(name.trim().toLowerCase())||name.trim();
}

interface SplitPayoutExportRow{phase:"held"|"pending"|"paid";itemName:string;mobName?:string;looterName?:string;valuePp:number;note?:string;happenedAt:string;attendees:string[];payouts:{name:string;paidAt:string}[]}

function splitPayoutExportRows(splits:Split[],history:History[]):SplitPayoutExportRow[]{
 const held=splits.map(row=>({phase:"held" as const,itemName:row.itemName,mobName:row.mobName,looterName:row.looterName,valuePp:row.payoutValuePp??row.marketValuePp??0,happenedAt:row.addedAt,attendees:row.attendees,payouts:[]}));
 const sold=history.filter(row=>row.disposition==="sold").map(row=>({phase:(row.payoutStatus==="completed"?"paid":"pending") as "paid"|"pending",itemName:row.itemName,mobName:row.mobName,looterName:row.looterName,valuePp:row.valuePp,note:row.note,happenedAt:row.completedAt,attendees:row.attendees,payouts:row.payouts||[]}));
 return [...held,...sold];
}

export function splitPayoutCompactMessages(splits:Split[],history:History[],aliases:Alias[],title:string):string[]{
 const rows=splitPayoutExportRows(splits,history),canonical=soldItemCanonicalizer(aliases);
 const rowLines=rows.map(row=>{
  const people=[...new Map(row.attendees.map(name=>canonical(name)).filter(Boolean).map(name=>[name.toLowerCase(),name])).values()];
  const valueLabel=row.phase==="held"?"Est. value":"Sale price";
  return "- **"+discordDetailCell(row.itemName)+"** - Phase: **"+row.phase+"** - "+valueLabel+": **"+discordMoney(row.valuePp)+"** - Split toons: "+discordDetailCell(people.join(", ")||"None");
 });
 return soldItemsDiscordMessages(title+" - Compact",rows,rowLines,"No split payout items in this phase.","total tracked value");
}

export function splitPayoutFullDetailMessages(splits:Split[],history:History[],aliases:Alias[],title:string):string[]{
 const rows=splitPayoutExportRows(splits,history),canonical=soldItemCanonicalizer(aliases);
 const rowLines=rows.map(row=>{
  const people=[...new Map(row.attendees.map(name=>canonical(name)).filter(Boolean).map(name=>[name.toLowerCase(),name])).values()];
  const paid=new Set(row.payouts.map(payout=>canonical(payout.name).toLowerCase()));
  const payouts=people.map(name=>name+" ["+(row.phase==="held"?"potential":paid.has(name.toLowerCase())?"paid":"pending")+"]").join(", ")||"None";
  const each=Math.floor(row.valuePp/Math.max(1,people.length)),valueLabel=row.phase==="held"?"Estimated value":"Sale value";
  return "- **"+discordDetailCell(row.itemName)+"** - Phase: **"+row.phase+"** - Dropped by: "+discordDetailCell(row.mobName)+" - Held / sold by: "+discordDetailCell(row.looterName)+" - Player payouts: "+discordDetailCell(payouts)+" - "+valueLabel+": **"+discordMoney(row.valuePp)+"** - Each payout: **"+discordMoney(each)+"** - Note: "+discordDetailCell(row.note)+" - Event date: "+discordDetailCell(row.happenedAt);
 });
 return soldItemsDiscordMessages(title+" - Full Detail",rows,rowLines,"No split payout items in this phase.","total tracked value");
}

export function soldItemsCompactMessages(history:History[],aliases:Alias[]):string[]{
 const rows=history.filter(row=>row.disposition==="sold"),canonical=soldItemCanonicalizer(aliases);
 const rowLines=rows.map(row=>{
  const people=[...new Map(row.attendees.map(name=>canonical(name)).filter(Boolean).map(name=>[name.toLowerCase(),name])).values()];
  return "- **"+discordDetailCell(row.itemName)+"** - Sale price: **"+discordMoney(row.valuePp)+"** - Split toons: "+discordDetailCell(people.join(", ")||"None");
 });
 return soldItemsDiscordMessages("Sold Split Items - Compact",rows,rowLines);
}

export function soldItemsFullDetailMessages(history:History[],aliases:Alias[]):string[]{
 const rows=history.filter(row=>row.disposition==="sold"),canonical=soldItemCanonicalizer(aliases);
 const rowLines=rows.map(row=>{
  const people=[...new Map(row.attendees.map(name=>canonical(name)).filter(Boolean).map(name=>[name.toLowerCase(),name])).values()];
  const each=Math.floor(row.valuePp/Math.max(1,people.length));
  const paid=new Set((row.payouts||[]).map(payout=>canonical(payout.name).toLowerCase()));
  const payouts=people.map(name=>name+" ["+(paid.has(name.toLowerCase())?"paid":"pending")+"]").join(", ")||"None";
  return "- **"+discordDetailCell(row.itemName)+"** - Dropped by: "+discordDetailCell(row.mobName)+" - Held / sold by: "+discordDetailCell(row.looterName)+" - Player payouts: "+discordDetailCell(payouts)+" - Sale value: **"+discordMoney(row.valuePp)+"** - Each payout: **"+discordMoney(each)+"** - Note: "+discordDetailCell(row.note)+" - Sold / consumed: "+discordDetailCell(row.completedAt);
 });
 return soldItemsDiscordMessages("Sold Split Items - Full Detail",rows,rowLines);
}
