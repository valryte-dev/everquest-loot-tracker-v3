import type {TimedClericHealCall} from "./model";

export type ChainPulseTone="stable"|"early"|"late";
export interface ChainPulsePoint { callNumber:number; gapSeconds:number; tone:ChainPulseTone }
export interface ChainPulseModel {
 points:ChainPulsePoint[];
 medianSeconds:number|null;
 targetSeconds:number|null;
 usesExpectedGap:boolean;
 toleranceSeconds:number;
 deviationSeconds:number|null;
 stabilityScore:number|null;
 stabilityLabel:"Learning"|"Locked in"|"Steady"|"Variable"|"Unstable";
 trend:"Learning"|"Steady"|"Speeding up"|"Slowing down";
 openGapSeconds:number;
 openGapTone:"learning"|"on-pace"|"due"|"overdue";
}
const mean=(values:number[])=>values.reduce((sum,value)=>sum+value,0)/Math.max(1,values.length);
const median=(values:number[])=>{const sorted=[...values].sort((a,b)=>a-b),middle=Math.floor(sorted.length/2);return sorted.length%2?sorted[middle]:(sorted[middle-1]+sorted[middle])/2};

export function buildChainPulse(calls:TimedClericHealCall[],openGapSeconds:number,limit=18,expectedGapSeconds?:number):ChainPulseModel{
 const samples=calls.flatMap(call=>call.gapSeconds===undefined?[]:[{callNumber:call.callNumber,gapSeconds:call.gapSeconds}]).slice(-limit);
 const expected=Number.isFinite(expectedGapSeconds)&&expectedGapSeconds!>0?expectedGapSeconds!:undefined;
 if(!samples.length)return {points:[],medianSeconds:null,targetSeconds:expected??null,usesExpectedGap:expected!==undefined,toleranceSeconds:expected?Math.max(1.25,expected*.2):0,deviationSeconds:null,stabilityScore:null,stabilityLabel:"Learning",trend:"Learning",openGapSeconds,openGapTone:expected===undefined?"learning":openGapSeconds>expected+Math.max(1.25,expected*.2)?"overdue":openGapSeconds>=Math.max(0,expected-Math.max(1.25,expected*.2))?"due":"on-pace"};
 const gaps=samples.map(sample=>sample.gapSeconds),learnedMedian=median(gaps),center=expected??learnedMedian,tolerance=Math.max(1.25,center*.2),average=mean(gaps);
 const deviation=Math.sqrt(mean(gaps.map(gap=>(gap-average)**2))),score=samples.length<2?null:Math.max(0,Math.min(100,Math.round(100-deviation/Math.max(1,center)*100)));
 const points=samples.map(sample=>({...sample,tone:Math.abs(sample.gapSeconds-center)<=tolerance?"stable" as const:sample.gapSeconds<center?"early" as const:"late" as const}));
 const recent=gaps.slice(-3),prior=gaps.slice(Math.max(0,gaps.length-6),Math.max(0,gaps.length-3)),trend=prior.length<2?"Learning":mean(recent)-mean(prior)>tolerance*.35?"Slowing down":mean(prior)-mean(recent)>tolerance*.35?"Speeding up":"Steady";
 const openGapTone=openGapSeconds>center+tolerance?"overdue":openGapSeconds>=Math.max(0,center-tolerance)?"due":"on-pace";
 const stabilityLabel=score===null?"Learning":score>=92?"Locked in":score>=80?"Steady":score>=62?"Variable":"Unstable";
 return {points,medianSeconds:learnedMedian,targetSeconds:center,usesExpectedGap:expected!==undefined,toleranceSeconds:tolerance,deviationSeconds:deviation,stabilityScore:score,stabilityLabel,trend,openGapSeconds,openGapTone};
}
