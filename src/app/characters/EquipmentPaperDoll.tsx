import {useEffect,useState} from "react";
import type {InventoryItem} from "../../shared/contracts";
import {CharacterModelViewer} from "./CharacterModelViewer";
import {characterClassesForRace,characterRacesForClass,isCharacterRaceClassCompatible,type CharacterModelProfile} from "./characterProfile";

export type EquipmentSlotKey=
 "left-ear"|"head"|"face"|"right-ear"|"neck"|"shoulders"|"arms"|"back"|
 "left-wrist"|"right-wrist"|"range"|"hands"|"primary"|"secondary"|"left-finger"|
 "right-finger"|"chest"|"legs"|"feet"|"waist"|"ammo";

export interface EquipmentSlot {key:EquipmentSlotKey;label:string}

export const PAPERDOLL_TOP_SLOTS:EquipmentSlot[]=[
 {key:"neck",label:"Neck"},{key:"face",label:"Face"},{key:"head",label:"Head"},
];
export const PAPERDOLL_LEFT_SLOTS:EquipmentSlot[]=[
 {key:"left-ear",label:"Left Ear"},{key:"chest",label:"Chest"},{key:"arms",label:"Arms"},
 {key:"left-wrist",label:"Left Wrist"},{key:"waist",label:"Waist"},{key:"legs",label:"Legs"},
 {key:"left-finger",label:"Left Finger"},{key:"range",label:"Range"},
];
export const PAPERDOLL_RIGHT_SLOTS:EquipmentSlot[]=[
 {key:"right-ear",label:"Right Ear"},{key:"back",label:"Back"},{key:"shoulders",label:"Shoulders"},
 {key:"right-wrist",label:"Right Wrist"},{key:"hands",label:"Hands"},{key:"feet",label:"Feet"},
 {key:"right-finger",label:"Right Finger"},{key:"ammo",label:"Ammo"},
];
export const PAPERDOLL_WEAPON_SLOTS:EquipmentSlot[]=[
 {key:"primary",label:"Primary"},{key:"secondary",label:"Secondary"},
];
export const PAPERDOLL_SLOT_GROUPS=[
 PAPERDOLL_TOP_SLOTS,PAPERDOLL_LEFT_SLOTS,PAPERDOLL_RIGHT_SLOTS,PAPERDOLL_WEAPON_SLOTS,
];

const SIMPLE_SLOTS:Record<string,EquipmentSlotKey>={
 head:"head",face:"face",neck:"neck",shoulder:"shoulders",shoulders:"shoulders",
 arms:"arms",back:"back",range:"range",ranged:"range",hands:"hands",primary:"primary",
 secondary:"secondary",chest:"chest",legs:"legs",feet:"feet",waist:"waist",ammo:"ammo",
};

export function equipmentSlotKey(location:string,occurrence=0):EquipmentSlotKey|undefined{
 const normalized=location.toLocaleLowerCase().replace(/[^a-z0-9]/g,"");
 if(SIMPLE_SLOTS[normalized])return SIMPLE_SLOTS[normalized];
 if(normalized==="ear")return occurrence%2===0?"left-ear":"right-ear";
 if(normalized==="wrist")return occurrence%2===0?"left-wrist":"right-wrist";
 if(normalized==="finger"||normalized==="fingers")return occurrence%2===0?"left-finger":"right-finger";
 if(/^(leftear|ear1|earleft)$/.test(normalized))return "left-ear";
 if(/^(rightear|ear2|earright)$/.test(normalized))return "right-ear";
 if(/^(leftwrist|wrist1|wristleft)$/.test(normalized))return "left-wrist";
 if(/^(rightwrist|wrist2|wristright)$/.test(normalized))return "right-wrist";
 if(/^(leftfinger|finger1|fingerleft)$/.test(normalized))return "left-finger";
 if(/^(rightfinger|finger2|fingerright)$/.test(normalized))return "right-finger";
 return undefined;
}

export function itemIconPath(iconId?:number):string|undefined{
 return iconId&&iconId>0?`/item-icons/Item_${iconId}.png`:undefined;
}

export function ItemNameWithIcon({item,compact=false}:{item:InventoryItem;compact?:boolean}){
 const icon=itemIconPath(item.iconId);
 return <span className={"inventory-item-name"+(compact?" compact":"")}>
  <span className="inventory-item-icon">{icon?<img src={icon} alt="" loading="lazy"/>:<i aria-hidden="true">?</i>}</span>
  <span><strong>{item.itemName.trim()||"Empty"}</strong>{!compact&&item.itemId&&<small>ID {item.itemId}</small>}</span>
 </span>;
}

function EquipmentSlotCard({slot,item,onSelect,active=false}:{slot:EquipmentSlot;item?:InventoryItem;onSelect?:(slot:EquipmentSlot)=>void;active?:boolean}){
 const occupied=Boolean(item?.itemName.trim());
 const content=<><span className="paperdoll-slot-label">{slot.label}</span>
  {occupied?<ItemNameWithIcon item={item!} compact/>:<span className="paperdoll-empty-icon" aria-hidden="true"/>}
  <span className="paperdoll-slot-detail">{occupied?(item!.valuePp?`${Math.round((item!.valuePp||0)*item!.count).toLocaleString()} pp`:"No estimate"):"Empty slot"}</span></>;
 const className="paperdoll-slot"+(occupied?" occupied":" empty")+(active?" active":"")+(onSelect?" selectable":"");
 const title=occupied?`${slot.label}: ${item!.itemName}`:`${slot.label}: Empty`;
 return onSelect?<button type="button" className={className} data-slot={slot.key} title={`${title}. Choose an item.`} onClick={()=>onSelect(slot)}>{content}</button>:<article className={className} data-slot={slot.key} title={title}>
  {content}
 </article>;
}

function SlotGroup({className,slots,items,onSlotClick,activeSlot}:{className:string;slots:EquipmentSlot[];items:Map<EquipmentSlotKey,InventoryItem>;onSlotClick?:(slot:EquipmentSlot)=>void;activeSlot?:EquipmentSlotKey}){
 return <div className={className}>{slots.map(slot=><EquipmentSlotCard key={slot.key} slot={slot} item={items.get(slot.key)} onSelect={onSlotClick} active={activeSlot===slot.key}/>)}</div>;
}

export function EquipmentPaperDollView({items,character,profile,onSlotClick,activeSlot}:{items:InventoryItem[];character:string;profile:CharacterModelProfile;onSlotClick?:(slot:EquipmentSlot)=>void;activeSlot?:EquipmentSlotKey}){
 const mapped=new Map<EquipmentSlotKey,InventoryItem>();
 const unmatched:InventoryItem[]=[];
 const slotOccurrences=new Map<string,number>();
 for(const item of items){
  const normalized=item.location.toLocaleLowerCase().replace(/[^a-z0-9]/g,"");
  const occurrence=slotOccurrences.get(normalized)||0;
  slotOccurrences.set(normalized,occurrence+1);
  const key=equipmentSlotKey(item.location,occurrence);
  if(key&&!mapped.has(key))mapped.set(key,item);else if(item.itemName.trim())unmatched.push(item);
 }
 return <>
  <div className="paperdoll-layout" aria-label="Equipped item slots">
   <SlotGroup className="paperdoll-rail paperdoll-rail-left" slots={PAPERDOLL_LEFT_SLOTS} items={mapped} onSlotClick={onSlotClick} activeSlot={activeSlot}/>
   <div className="paperdoll-center">
    <SlotGroup className="paperdoll-crown" slots={PAPERDOLL_TOP_SLOTS} items={mapped} onSlotClick={onSlotClick} activeSlot={activeSlot}/>
    <section className="character-model-stage" aria-label={`Character model area for ${character}`}>
     <header><span>Character model</span><strong>{character||"No character selected"}</strong></header>
     <CharacterModelViewer character={character} profile={profile} items={items}/>
     <footer><span>{mapped.size} / 21 slots equipped</span><span>Interactive EverQuest model</span></footer>
    </section>
    <SlotGroup className="paperdoll-weapons" slots={PAPERDOLL_WEAPON_SLOTS} items={mapped} onSlotClick={onSlotClick} activeSlot={activeSlot}/>
   </div>
   <SlotGroup className="paperdoll-rail paperdoll-rail-right" slots={PAPERDOLL_RIGHT_SLOTS} items={mapped} onSlotClick={onSlotClick} activeSlot={activeSlot}/>
  </div>
  {unmatched.length>0&&<div className="paperdoll-unmatched"><strong>Other equipped records</strong>{unmatched.map(item=><ItemNameWithIcon key={item.id} item={item} compact/>)}</div>}
 </>;
}

export function EquipmentPaperDoll({items,character,profile,level,parsedLevel,levelSource,onProfileChange,onLevelChange}:{items:InventoryItem[];character:string;profile:CharacterModelProfile;level?:number;parsedLevel?:number;levelSource:"logs"|"manual"|"unknown";onProfileChange:(profile:CharacterModelProfile)=>void;onLevelChange:(level?:number)=>void}){
 const[levelDraft,setLevelDraft]=useState(level?String(level):"");
 useEffect(()=>setLevelDraft(level?String(level):""),[character,level]);
 const parsedDraft=Number(levelDraft),validLevel=Number.isInteger(parsedDraft)&&parsedDraft>=1&&parsedDraft<=255;
 const raceOptions=characterRacesForClass(profile.classCode);
 const classOptions=characterClassesForRace(profile.race);
 return <div className="equipment-paperdoll">
  <section className="character-profile-editor" aria-label={"Saved character details for "+character}>
   <label><span>Race</span><select value={profile.race} onChange={event=>{const race=event.target.value;onProfileChange({...profile,race,classCode:isCharacterRaceClassCompatible(race,profile.classCode)?profile.classCode:""})}}>{raceOptions.map(race=><option key={race.code} value={race.code}>{race.name}</option>)}</select></label>
   <label><span>Class</span><select value={profile.classCode} onChange={event=>onProfileChange({...profile,classCode:event.target.value})}>{classOptions.map(entry=><option key={entry.code||"unknown"} value={entry.code}>{entry.name}</option>)}</select></label>
   <label><span>Gender</span><select value={profile.gender} onChange={event=>onProfileChange({...profile,gender:event.target.value as "m"|"f"})}><option value="m">Male</option><option value="f">Female</option></select></label>
   <label className="character-level-field"><span>Level</span><input type="number" min="1" max="255" inputMode="numeric" value={levelDraft} placeholder="Unknown" onChange={event=>setLevelDraft(event.target.value)}/></label>
   <button className="primary" disabled={!validLevel||parsedDraft===level} onClick={()=>onLevelChange(parsedDraft)}>Save level</button>
   {levelSource==="manual"&&<button title={parsedLevel?`Return to parsed level ${parsedLevel}`:"Remove the manual level"} onClick={()=>onLevelChange(undefined)}>{parsedLevel?`Use parsed ${parsedLevel}`:"Clear override"}</button>}
   <small>{levelSource==="logs"?"Level from the latest parsed log event":levelSource==="manual"?"Manual level override":"No level event has been parsed yet"}</small>
  </section>
  <EquipmentPaperDollView items={items} character={character} profile={profile}/>
 </div>;
}
