import {useEffect,useMemo,useState} from "react";
import {openUrl} from "@tauri-apps/plugin-opener";
import type {AppSnapshot,WardrobeCatalogItem} from "../../shared/contracts";
import {getWardrobeCatalogItems} from "../../shared/backend";
import {DataTable,Modal,type Column} from "../ui";
import {CHARACTER_CLASSES,CHARACTER_RACES} from "../characters/characterProfile";
import {WARDROBE_STORAGE_KEY} from "../wardrobe/characterTransfer";
import {classBit,raceBit,SLOT_BITS,weaponHandedness,weaponRatio} from "../wardrobe/model";
import {loadWardrobeDpsEquipment,resolveCharacterDpsEquipment,type DpsEquipmentLoadout} from "./equipment";
import {calculateDps,DPS_CLASSES,suggestedSkills,type DpsCalculatorInput,type DpsClass} from "./model";
import {classesForDpsRace,DPS_RACES,racesForDpsClass,startingStatsFor} from "./startingStats";

const DEFAULT_RACE="Human";
const DEFAULT_INPUT:DpsCalculatorInput={characterClass:"Warrior",level:60,strength:startingStatsFor(DEFAULT_RACE,"Warrior")?.strength||85,haste:0,dualWieldSkill:252,doubleAttackSkill:252,offenseSkill:252,backstabSkill:0,mainDamage:10,mainDelay:20,mainHand:"1h",offDamage:0,offDelay:0};
const normalize=(value:string)=>value.replace(/[\s-]/g,"").toLocaleLowerCase();
const classByCode=new Map<string,DpsClass|undefined>(CHARACTER_CLASSES.filter(entry=>entry.code).map(entry=>[entry.code,DPS_CLASSES.find(name=>normalize(name)===normalize(entry.name))]));
const classCodeByName=new Map<DpsClass,string>(DPS_CLASSES.map(name=>[name,CHARACTER_CLASSES.find(entry=>normalize(entry.name)===normalize(name))?.code||""]));
const raceNameByCode=new Map<string,string>(CHARACTER_RACES.map(entry=>[entry.code,entry.name]));
const raceCodeByName=new Map<string,string>(CHARACTER_RACES.map(entry=>[entry.name,entry.code]));
const pct=(value:number)=>`${(value*100).toFixed(1)}%`;

function NumberField({label,value,onChange,min=0,max,step=1,help}:{label:string;value:number;onChange:(value:number)=>void;min?:number;max?:number;step?:number;help?:string}){
 return <label><span>{label}</span><input type="number" value={value} min={min} max={max} step={step} onChange={event=>onChange(Number(event.target.value))}/>{help&&<small>{help}</small>}</label>;
}

function WeaponPicker({hand,race,characterClass,close,choose}:{hand:"primary"|"secondary";race:string;characterClass:DpsClass;close:()=>void;choose:(item:WardrobeCatalogItem)=>void}){
 const[items,setItems]=useState<WardrobeCatalogItem[]>([]),[loading,setLoading]=useState(true),[error,setError]=useState("");
 useEffect(()=>{
  let active=true;setLoading(true);setError("");
  const slotBit=SLOT_BITS[hand],classCode=classCodeByName.get(characterClass)||"",raceCode=raceCodeByName.get(race)||"";
  getWardrobeCatalogItems(slotBit,classBit(classCode),raceBit(raceCode)).then(rows=>{
   if(!active)return;
   setItems(rows.filter(item=>item.damage>0&&item.delay>0&&(hand==="primary"||weaponHandedness(item)==="1h")));
  }).catch(value=>active&&setError(String(value))).finally(()=>active&&setLoading(false));
  return()=>{active=false};
 },[hand,race,characterClass]);
 const columns:Column<WardrobeCatalogItem>[]=[
  {key:"name",label:"Weapon",value:item=>item.name,render:item=><span className="dps-weapon-choice-name"><strong>{item.name}</strong>{item.procName&&<small>Proc: {item.procName}</small>}</span>},
  {key:"damage",label:"DMG",value:item=>item.damage},
  {key:"delay",label:"DLY",value:item=>item.delay},
  {key:"ratio",label:"Ratio",value:weaponRatio,render:item=>weaponRatio(item).toFixed(3)},
  {key:"hand",label:"Type",value:item=>weaponHandedness(item)||"1h"},
  {key:"stats",label:"Useful stats",value:item=>[item.strength,item.dexterity,item.attack,item.haste].join("/"),render:item=><span>STR {item.strength} / DEX {item.dexterity}<small className="wardrobe-substat">ATK {item.attack} / HST {item.haste}</small></span>},
 ];
 return <Modal title={`Choose ${hand==="primary"?"main-hand":"off-hand"} weapon`} onClose={close} footer={<button onClick={close}>Close</button>}><div className="dps-weapon-picker"><p>{race} {characterClass} weapons valid for the {hand} slot. Filter by name, proc, stats, damage, delay, or ratio, then click a row.</p>{error?<div className="alert"><strong>Weapon catalog unavailable</strong><span>{error}</span></div>:loading?<div className="wardrobe-loading"><span className="spinner"/><p>Loading weapons...</p></div>:<DataTable rows={items} columns={columns} rowKey={item=>item.id} onRowClick={choose} empty="No compatible weapons found."/>}</div></Modal>;
}

export function DpsCalculatorPage({data}:{data:AppSnapshot}){
 const[input,setInput]=useState(DEFAULT_INPUT),[race,setRace]=useState(DEFAULT_RACE),[gearStrength,setGearStrength]=useState(0),[source,setSource]=useState(data.settings.active_character||"manual"),[loading,setLoading]=useState(false),[loadMessage,setLoadMessage]=useState("");
 const[picker,setPicker]=useState<"primary"|"secondary"|null>(null),[primary,setPrimary]=useState<WardrobeCatalogItem|undefined>(),[secondary,setSecondary]=useState<WardrobeCatalogItem|undefined>();
 const characters=useMemo(()=>[...new Set([...data.characterProfiles.map(row=>row.character),...data.inventory.map(row=>row.character)])].sort((a,b)=>a.localeCompare(b)),[data.characterProfiles,data.inventory]);
 const calculated=useMemo(()=>calculateDps({...input,strength:input.strength+gearStrength}),[input,gearStrength]);
 const starting=startingStatsFor(race,input.characterClass);
 const set=<K extends keyof DpsCalculatorInput>(key:K,value:DpsCalculatorInput[K])=>setInput(current=>({...current,[key]:value}));
 const applyCombination=(nextRace:string,nextClass:DpsClass)=>{
  const stats=startingStatsFor(nextRace,nextClass);
  setRace(nextRace);
  setInput(current=>({...current,characterClass:nextClass,strength:stats?.strength??current.strength,...suggestedSkills(nextClass,current.level)}));
 };
 const changeRace=(nextRace:string)=>{
  const allowed=classesForDpsRace(nextRace,DPS_CLASSES);
  applyCombination(nextRace,allowed.includes(input.characterClass)?input.characterClass:allowed[0]);
 };
 const resetRaceClass=()=>applyCombination(DEFAULT_RACE,"Warrior");
 const applyLoadout=(loadout:DpsEquipmentLoadout,characterClass?:DpsClass,level?:number,raceName?:string)=>{
  const nextClass=characterClass||input.characterClass,nextRace=raceName&&startingStatsFor(raceName,nextClass)?raceName:race;
  const stats=startingStatsFor(nextRace,nextClass),main=loadout.primary,off=loadout.mainHand==="2h"?undefined:loadout.secondary;
  setRace(nextRace);setGearStrength(loadout.gearStrength);setPrimary(main);setSecondary(off);
  setInput(current=>({...current,characterClass:nextClass,strength:stats?.strength??current.strength,level:level||current.level,...suggestedSkills(nextClass,level||current.level),haste:loadout.haste,mainDamage:main?.damage||0,mainDelay:main?.delay||0,mainHand:loadout.mainHand,offDamage:off?.damage||0,offDelay:off?.delay||0}));
  const names=[main?.name&&`Primary: ${main.name}`,off?.name&&`Secondary: ${off.name}`].filter(Boolean).join(" / ");
  setLoadMessage(`${names||"No equipped weapons found"}. Gear STR +${loadout.gearStrength}, worn haste ${loadout.haste}%.${loadout.missing.length?` ${loadout.missing.length} equipped item(s) were not found in the catalog.`:""}`);
 };
 const loadSource=async()=>{
  if(source==="manual"){setGearStrength(0);setLoadMessage("Manual inputs enabled. Choose race, class, and weapons below.");return}
  setLoading(true);setLoadMessage("");
  try{
   if(source==="wardrobe"){
    const result=loadWardrobeDpsEquipment(localStorage.getItem(WARDROBE_STORAGE_KEY));
    if(!result.loadout||!result.draft){setLoadMessage("No saved Wardrobe outfit is available yet.");return}
    applyLoadout(result.loadout,classByCode.get(result.draft.profile.classCode),undefined,raceNameByCode.get(result.draft.profile.race));return;
   }
   const profile=data.characterProfiles.find(row=>row.character.toLocaleLowerCase()===source.toLocaleLowerCase());
   const inventory=data.inventory.filter(row=>row.character.toLocaleLowerCase()===source.toLocaleLowerCase());
   const loadout=await resolveCharacterDpsEquipment(inventory);
   applyLoadout(loadout,profile?classByCode.get(profile.classCode):undefined,profile?.level,raceNameByCode.get(profile?.race||""));
  }catch(error){setLoadMessage(`Could not load equipment: ${String(error)}`)}finally{setLoading(false)}
 };
 const chooseWeapon=(item:WardrobeCatalogItem)=>{
  const handed=weaponHandedness(item)||"1h";
  if(picker==="primary"){
   setPrimary(item);if(handed==="2h")setSecondary(undefined);
   setInput(current=>({...current,mainDamage:item.damage,mainDelay:item.delay,mainHand:handed,offDamage:handed==="2h"?0:current.offDamage,offDelay:handed==="2h"?0:current.offDelay}));
  }else{
   setSecondary(item);setInput(current=>({...current,offDamage:item.damage,offDelay:item.delay}));
  }
  setPicker(null);
 };
 const clearWeapon=(hand:"primary"|"secondary")=>{
  if(hand==="primary"){setPrimary(undefined);setInput(current=>({...current,mainDamage:0,mainDelay:0,mainHand:"1h"}))}
  else{setSecondary(undefined);setInput(current=>({...current,offDamage:0,offDelay:0}))}
 };
 const useCaps=()=>setInput(current=>({...current,...suggestedSkills(current.characterClass,current.level)}));
 const mainShare=calculated.totalDps?Math.min(100,(calculated.main.baseDps/calculated.totalDps)*100):0;
 const offShare=calculated.totalDps?Math.min(100,(calculated.off.baseDps/calculated.totalDps)*100):0;
 return <section className="dps-calc-page">
  <section className="card dps-calc-source"><header><div><span className="eyebrow">Local equipment integration</span><h2>Populate from your roster</h2><p>Load known race, class, level, worn haste, gear Strength and weapons, then adjust anything manually.</p></div></header><div><label><span>Setup source</span><select value={source} onChange={event=>setSource(event.target.value)}><option value="manual">Manual entry</option><option value="wardrobe">Current Wardrobe outfit</option>{characters.map(character=><option key={character} value={character}>{character}</option>)}</select></label><button className="primary" disabled={loading} onClick={loadSource}>{loading?"Loading...":"Load setup"}</button>{loadMessage&&<p role="status">{loadMessage}</p>}</div></section>
  <div className="dps-calc-layout"><section className="card dps-calc-inputs"><header><div><span className="eyebrow">Character & skills</span><h2>Combat inputs</h2></div><button onClick={useCaps}>Use suggested skill caps</button></header>
   <div className="dps-identity-grid"><label><span>Race</span><select value={race} onChange={event=>changeRace(event.target.value)}>{DPS_RACES.map(name=><option key={name}>{name}</option>)}</select></label><label><span>Class</span><select value={input.characterClass} onChange={event=>applyCombination(race,event.target.value as DpsClass)}>{classesForDpsRace(race,DPS_CLASSES).map(name=><option key={name}>{name}</option>)}</select></label><button onClick={resetRaceClass}>Reset race / class</button><small>{racesForDpsClass(input.characterClass).length} playable races for {input.characterClass}</small></div>
   {starting&&<div className="dps-starting-stats"><span>Default starting stats</span>{([["STR",starting.strength],["STA",starting.stamina],["AGI",starting.agility],["DEX",starting.dexterity],["WIS",starting.wisdom],["INT",starting.intelligence],["CHA",starting.charisma],["Bonus",starting.bonusPoints]] as const).map(([label,value])=><div key={label}><small>{label}</small><strong>{value}</strong></div>)}</div>}
   <div className="dps-field-grid"><NumberField label="Level" value={input.level} min={1} max={60} onChange={value=>set("level",value)}/><NumberField label="STR before gear" value={input.strength} max={255} onChange={value=>set("strength",value)} help={`Starts at ${starting?.strength??"?"}; gear +${gearStrength}; total ${Math.min(255,input.strength+gearStrength)}`}/><NumberField label="Haste %" value={input.haste} max={100} onChange={value=>set("haste",value)} help={`Level cap ${pct(calculated.hasteCap)}`}/><NumberField label="Dual wield" value={input.dualWieldSkill} max={252} onChange={value=>set("dualWieldSkill",value)}/><NumberField label="Double attack" value={input.doubleAttackSkill} max={252} onChange={value=>set("doubleAttackSkill",value)}/><NumberField label="Offense" value={input.offenseSkill} max={252} onChange={value=>set("offenseSkill",value)}/><NumberField label="Backstab" value={input.backstabSkill} max={252} onChange={value=>set("backstabSkill",value)}/></div>
   <div className="dps-weapon-inputs"><section><header><div><strong>Main hand</strong>{primary&&<small>{primary.name}</small>}</div><div><button onClick={()=>setPicker("primary")}>Choose weapon</button>{primary&&<button onClick={()=>clearWeapon("primary")}>Clear</button>}<select value={input.mainHand} onChange={event=>set("mainHand",event.target.value as "1h"|"2h")}><option value="1h">1H</option><option value="2h">2H</option></select></div></header><NumberField label="Damage" value={input.mainDamage} max={200} onChange={value=>set("mainDamage",value)}/><NumberField label="Delay" value={input.mainDelay} max={200} onChange={value=>set("mainDelay",value)}/></section><section className={input.mainHand==="2h"?"disabled":undefined}><header><div><strong>Off hand</strong>{secondary&&<small>{secondary.name}</small>}</div><div><button onClick={()=>setPicker("secondary")}>Choose weapon</button>{secondary&&<button onClick={()=>clearWeapon("secondary")}>Clear</button>}</div></header><NumberField label="Damage" value={input.mainHand==="2h"?0:input.offDamage} max={200} onChange={value=>set("offDamage",value)}/><NumberField label="Delay" value={input.mainHand==="2h"?0:input.offDelay} max={200} onChange={value=>set("offDelay",value)}/></section></div></section>
   <section className="card dps-calc-results"><header><div><span className="eyebrow">Middle-line estimate</span><h2>Projected melee DPS</h2></div><strong className="dps-total">{calculated.totalDps.toFixed(2)}</strong></header><div className="dps-hand-results"><article><span>Main hand</span><strong>{calculated.main.baseDps.toFixed(2)} DPS</strong><small>{calculated.main.minimum}-{calculated.main.maximum} damage / {calculated.main.effectiveDelay} delay</small><i style={{width:`${mainShare}%`}}/></article><article><span>Off hand</span><strong>{calculated.off.baseDps.toFixed(2)} DPS</strong><small>{calculated.off.minimum}-{calculated.off.maximum} damage / {calculated.off.effectiveDelay} delay</small><i style={{width:`${offShare}%`}}/></article></div><dl className="dps-mechanics"><div><dt>Damage bonus</dt><dd>{calculated.damageBonus}</dd></div><div><dt>Damage modifier</dt><dd>{calculated.damageModifier.toFixed(2)}</dd></div><div><dt>Effective haste</dt><dd>{pct(calculated.effectiveHaste)}</dd></div><div><dt>Dual wield chance</dt><dd>{pct(calculated.dualWieldChance)}</dd></div><div><dt>Double attack</dt><dd>{pct(calculated.doubleAttackChance)}</dd></div><div><dt>Triple attack</dt><dd>{pct(calculated.tripleAttackChance)}</dd></div><div><dt>Main damage cap</dt><dd>{calculated.main.damageCap}</dd></div><div><dt>Off damage cap</dt><dd>{calculated.off.damageCap}</dd></div><div><dt>Max backstab</dt><dd>{calculated.maxBackstab}</dd></div></dl>{calculated.warnings.map(warning=><div className="dps-warning" key={warning}>{warning}</div>)}</section></div>
  <section className="card dps-calc-notes"><header><div><span className="eyebrow">Method & limitations</span><h2>What this estimate means</h2></div></header><div><ul><li>Estimates average landed melee damage; misses, target AC, attack rating, disciplines and procs are excluded.</li><li>Main-hand DPS is the base swing estimate. Total DPS adds applicable dual-wield, double-attack and triple-attack chances.</li><li>Equipped haste uses the highest worn haste item because worn haste does not stack.</li><li>Backstab is informational and is not included in total DPS.</li></ul><p>Native TypeScript adaptation of the reusable calculator source, integrated with the app equipment data. Source: <button className="dps-source-link" onClick={()=>openUrl("https://www.jklein.me/eqcalc/")}>jklein.me EQ DPS Calculator</button>.</p></div></section>
  {picker&&<WeaponPicker hand={picker} race={race} characterClass={input.characterClass} close={()=>setPicker(null)} choose={chooseWeapon}/>}
 </section>;
}
