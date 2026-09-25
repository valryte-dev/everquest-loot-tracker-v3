/**
 * Native typed adaptation of the reusable jklein.me P99 EQ DPS calculator.
 * The pure model replaces DOM/global state and adds explicit weapon handedness and enforced damage caps.
 * Source: https://www.jklein.me/eqcalc/js/scripts.js
 */
import {TWO_HAND_DAMAGE_BONUS_TABLE} from "./twoHandBonus";

export const DPS_CLASSES=["Bard","Cleric","Druid","Enchanter","Magician","Monk","Necromancer","Paladin","Ranger","Rogue","Shadow Knight","Shaman","Warrior","Wizard"] as const;
export type DpsClass=(typeof DPS_CLASSES)[number];
export type WeaponHand="1h"|"2h";
export interface DpsCalculatorInput {characterClass:DpsClass;level:number;strength:number;haste:number;dualWieldSkill:number;doubleAttackSkill:number;offenseSkill:number;backstabSkill:number;mainDamage:number;mainDelay:number;mainHand:WeaponHand;offDamage:number;offDelay:number}
export interface DpsHandResult {damageCap:number;minimum:number;maximum:number;effectiveDelay:number;baseDps:number}
export interface DpsCalculatorResult {main:DpsHandResult;off:DpsHandResult;totalDps:number;damageBonus:number;damageModifier:number;hasteCap:number;effectiveHaste:number;dualWieldChance:number;doubleAttackChance:number;tripleAttackChance:number;maxBackstab:number;warnings:string[]}

const MELEE_CLASSES=new Set<DpsClass>(["Bard","Monk","Paladin","Ranger","Rogue","Shadow Knight","Warrior"]);
const CASTER_CLASSES=new Set<DpsClass>(["Enchanter","Magician","Necromancer","Wizard"]);
const PRIEST_CLASSES=new Set<DpsClass>(["Cleric","Druid","Shaman"]);
const clamp=(value:number,min:number,max:number)=>Math.min(max,Math.max(min,Number.isFinite(value)?value:min));
const round2=(value:number)=>Math.round(value*100)/100;

export function hasteCap(level:number){return level<31?0.5:level<51?0.74:level<60?0.94:1}

export function weaponDamageCap(characterClass:DpsClass,level:number,damage:number){
 const value=Math.max(0,damage);
 const caps=CASTER_CLASSES.has(characterClass)?[6,10,12,18,20]:PRIEST_CLASSES.has(characterClass)?[9,12,20,26,40]:[10,20,30,60,100];
 const cap=level<10?caps[0]:level<20?caps[1]:level<30?caps[2]:level<40?caps[3]:caps[4];
 return Math.min(value,cap);
}

function twoHandDamageBonus(level:number,delay:number){
 if(level<28)return 0;
 const levelIndex=clamp(Math.floor(level),28,60)-28;
 const roundedDelay=Math.round(delay);
 const row=roundedDelay>=28&&roundedDelay<=60?roundedDelay-28:roundedDelay===70?33:roundedDelay===85?34:roundedDelay===95?35:roundedDelay===150?36:-1;
 return row>=0?TWO_HAND_DAMAGE_BONUS_TABLE[row]?.[levelIndex]??0:Math.floor((level-25)/3);
}

export function damageBonus(input:DpsCalculatorInput){
 if(input.level<28||!MELEE_CLASSES.has(input.characterClass)||input.mainDamage<=0)return 0;
 return input.mainHand==="2h"?twoHandDamageBonus(input.level,input.mainDelay):Math.floor((input.level-25)/3);
}

export function suggestedSkills(characterClass:DpsClass,level:number){
 const cap=Math.min(252,Math.max(0,Math.floor(level)*5+5));
 const dual=new Set<DpsClass>(["Bard","Monk","Ranger","Rogue","Warrior"]).has(characterClass)?cap:0;
 const doubleAttack=new Set<DpsClass>(["Monk","Paladin","Ranger","Rogue","Shadow Knight","Warrior"]).has(characterClass)?cap:0;
 return{dualWieldSkill:dual,doubleAttackSkill:doubleAttack,offenseSkill:cap,backstabSkill:characterClass==="Rogue"?cap:0};
}

export function calculateDps(input:DpsCalculatorInput):DpsCalculatorResult{
 const level=clamp(Math.floor(input.level),1,60),strength=clamp(input.strength,0,255),cap=hasteCap(level);
 const effectiveHaste=Math.min(clamp(input.haste,0,100)/100,cap);
 const effectiveDelay=(damage:number,delay:number)=>damage<=0?0:Math.max(5,Math.round(Math.max(1,delay)/(1+effectiveHaste)));
 const modifier=Math.max(2,(Math.max(0,input.offenseSkill)+strength)/100),bonus=damageBonus({...input,level,strength});
 const mainDamage=weaponDamageCap(input.characterClass,level,input.mainDamage),offDamage=input.mainHand==="2h"?0:weaponDamageCap(input.characterClass,level,input.offDamage);
 const mainDelay=effectiveDelay(mainDamage,input.mainDelay),offDelay=effectiveDelay(offDamage,input.offDelay);
 const mainMinimum=mainDamage>0?bonus+1:0,mainMaximum=mainDamage>0?Math.floor(mainDamage*modifier+bonus):0;
 const offMinimum=offDamage>0?1:0,offMaximum=offDamage>0?Math.floor(offDamage*modifier):0;
 const dualDivisor=input.characterClass==="Monk"?400:new Set<DpsClass>(["Warrior","Rogue","Ranger","Bard"]).has(input.characterClass)?500:Infinity;
 const dualWieldChance=Number.isFinite(dualDivisor)?clamp((level+Math.max(0,input.dualWieldSkill))/dualDivisor,0,1):0;
 const doubleAttackChance=input.characterClass==="Bard"?0:clamp(Math.max(0,input.doubleAttackSkill)/(400*1.05),0,1);
 const tripleAttackChance=level>=60&&(input.characterClass==="Monk"||input.characterClass==="Warrior")?clamp((Math.max(0,input.doubleAttackSkill)/2)/(400*1.05),0,1):0;
 const handDps=(minimum:number,maximum:number,delay:number)=>delay>0?((minimum+maximum)/2)/(delay*.1):0;
 const mainBase=handDps(mainMinimum,mainMaximum,mainDelay),offBase=handDps(offMinimum,offMaximum,offDelay)*dualWieldChance;
 let total=mainBase+mainBase*doubleAttackChance+offBase;
 if(input.dualWieldSkill>=150)total+=offBase*doubleAttackChance;
 total+=mainBase*tripleAttackChance;
 const strUnder=Math.min(strength,200),strOver=Math.max(0,strength-200);
 const maxBackstab=input.characterClass==="Rogue"?Math.round(((Math.max(0,input.offenseSkill)+strUnder)+(strOver/5))*mainDamage*(2+Math.max(0,input.backstabSkill)*.02)/100):0;
 const warnings:string[]=[];
 if(input.mainHand==="2h"&&input.mainDelay>60&&![70,85,95,150].includes(Math.round(input.mainDelay)))warnings.push("The published P99 two-hand bonus table has no exact row for this delay; the 1H level bonus is used as a fallback.");
 if(input.mainDamage>mainDamage||(input.mainHand!=="2h"&&input.offDamage>offDamage))warnings.push("One or more weapon damage values were reduced by the level/class damage cap.");
 return{main:{damageCap:mainDamage,minimum:mainMinimum,maximum:mainMaximum,effectiveDelay:mainDelay,baseDps:round2(mainBase)},off:{damageCap:offDamage,minimum:offMinimum,maximum:offMaximum,effectiveDelay:offDelay,baseDps:round2(offBase)},totalDps:round2(total),damageBonus:bonus,damageModifier:round2(modifier),hasteCap:cap,effectiveHaste,dualWieldChance,doubleAttackChance,tripleAttackChance,maxBackstab,warnings};
}