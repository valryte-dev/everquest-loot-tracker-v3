import type {InventoryItem,Spell,SpellInfo} from "../../shared/contracts";

export type CharacterSpellStatus="scribed"|"scroll"|"missing";

export interface CharacterSpellRow{
 info:SpellInfo;
 level:number;
 status:CharacterSpellStatus;
 bookSlot?:number;
 scrollCount:number;
}

export const spellIconPath=(letter:string)=>/^[A-V]$/i.test(letter.trim())?`/spell-icons/Spellicon_${letter.trim().toUpperCase()}.png`:undefined;

export const normalizeSpellName=(value:string)=>value
 .trim()
 .replace(/^spell:\s*/i,"")
 .replace(/[`‘’´]/g,"'")
 .replace(/\.$/,"")
 .replace(/\s+/g," ")
 .toLocaleLowerCase();

export function buildCharacterSpellRows(catalog:SpellInfo[],className:string,knownSpells:Spell[],inventory:InventoryItem[]):CharacterSpellRow[]{
 const normalizedClass=className.trim().toLocaleLowerCase();
 if(!normalizedClass)return[];
 const known=new Map(knownSpells.map(spell=>[normalizeSpellName(spell.spellName),spell]));
 const scrolls=new Map<string,number>();
 inventory.filter(item=>/^spell:\s*/i.test(item.itemName)).forEach(item=>{
  const key=normalizeSpellName(item.itemName);
  scrolls.set(key,(scrolls.get(key)||0)+item.count);
 });
 const rows=new Map<string,CharacterSpellRow>();
 catalog.forEach(info=>{
  const classInfo=info.classes.find(entry=>entry.name.toLocaleLowerCase()===normalizedClass);
  if(!classInfo)return;
  const key=normalizeSpellName(info.spellName),knownSpell=known.get(key),scrollCount=scrolls.get(key)||0;
  const row={info,level:classInfo.level,status:knownSpell?"scribed":scrollCount?"scroll":"missing",bookSlot:knownSpell?.slot,scrollCount} satisfies CharacterSpellRow;
  const previous=rows.get(key);
  if(!previous||row.level<previous.level)rows.set(key,row);
 });
 return [...rows.values()].sort((a,b)=>a.level-b.level||a.info.spellName.localeCompare(b.info.spellName));
}