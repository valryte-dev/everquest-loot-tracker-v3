import type {MerchantListingItem,MerchantMessage} from "../shared/contracts";

export type ArbitrageDirection="buy-resell"|"sell-to-buyer";
export type ArbitrageConfidence="high"|"medium"|"low";
export type ArbitrageSignal="strong"|"good"|"thin";

export interface ArbitrageScore{
 direction:ArbitrageDirection;
 quotedPricePp:number;
 marketPricePp:number;
 potentialProfitPp:number;
 marginPct:number;
 confidence:ArbitrageConfidence;
 signal:ArbitrageSignal;
 score:number;
}

export interface ArbitrageOpportunity extends ArbitrageScore{
 key:string;
 messageId:number;
 itemId:number;
 itemName:string;
 trader:string;
 happenedAt:string;
 count30d:number;
 marketValueBasis?:string;
}

export function scoreArbitrage(kind:MerchantMessage["kind"],item:MerchantListingItem):ArbitrageScore|null{
 if(kind==="tell"||item.askingPricePp==null||item.marketValuePp==null)return null;
 const quotedPricePp=item.askingPricePp,marketPricePp=item.marketValuePp;
 if(quotedPricePp<=0||marketPricePp<=0)return null;
 const direction:ArbitrageDirection=kind==="wts"?"buy-resell":"sell-to-buyer";
 const potentialProfitPp=kind==="wts"?marketPricePp-quotedPricePp:quotedPricePp-marketPricePp;
 if(potentialProfitPp<=0)return null;
 const basis=kind==="wts"?quotedPricePp:marketPricePp;
 const marginPct=potentialProfitPp/basis*100;
 const count=item.marketCount30d||0;
 const confidence:ArbitrageConfidence=count>=10?"high":count>=3?"medium":"low";
 const signal:ArbitrageSignal=marginPct>=20&&potentialProfitPp>=100&&confidence!=="low"?"strong":marginPct>=8&&potentialProfitPp>=50?"good":"thin";
 const liquidityWeight=confidence==="high"?1:confidence==="medium"?.75:.4;
 const signalWeight=signal==="strong"?1.35:signal==="good"?1:.55;
 return{direction,quotedPricePp,marketPricePp,potentialProfitPp,marginPct,confidence,signal,score:potentialProfitPp*(1+Math.min(marginPct,100)/100)*liquidityWeight*signalWeight};
}

export function identifyArbitrage(messages:MerchantMessage[]):ArbitrageOpportunity[]{
 const latest=new Map<string,ArbitrageOpportunity>();
 for(const message of messages){for(const item of message.items){const result=scoreArbitrage(message.kind,item);if(!result)continue;const opportunity={...result,key:`${message.id}-${item.id}`,messageId:message.id,itemId:item.id,itemName:item.itemName,trader:message.speakerName,happenedAt:message.happenedAt,count30d:item.marketCount30d||0,marketValueBasis:item.marketValueBasis};const identity=`${result.direction}|${message.speakerName.toLowerCase()}|${item.itemName.toLowerCase()}`,previous=latest.get(identity);if(!previous||new Date(opportunity.happenedAt).valueOf()>new Date(previous.happenedAt).valueOf())latest.set(identity,opportunity)}}
 return[...latest.values()].sort((left,right)=>right.score-left.score||right.potentialProfitPp-left.potentialProfitPp);
}
