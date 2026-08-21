export type ArmorArchetype="Melee"|"Priest"|"Caster";
export type ArmorSlot="Chest"|"Legs"|"Arms"|"Helm"|"Gloves"|"Boots"|"Bracer";

export interface GemUsage { archetype:ArmorArchetype; slot:ArmorSlot }
export interface VeliousArmorGem { name:string; icon:string; usages:GemUsage[] }

export const ARMOR_ARCHETYPES:ArmorArchetype[]=["Melee","Priest","Caster"];
export const ARMOR_SLOTS:ArmorSlot[]=["Chest","Legs","Arms","Helm","Gloves","Boots","Bracer"];
export const VELIOUS_GEM_SOURCE="https://wiki.project1999.com/Velious_Armor_Gems";

export const VELIOUS_ARMOR_GEMS:VeliousArmorGem[]=[
 {name:"Flawless Diamond",icon:"/gems/Item_966.png",usages:[{archetype:"Melee",slot:"Chest"}]},
 {name:"Flawed Sea Sapphire",icon:"/gems/Item_963.png",usages:[{archetype:"Melee",slot:"Legs"}]},
 {name:"Flawed Emerald",icon:"/gems/Item_958.png",usages:[{archetype:"Melee",slot:"Arms"}]},
 {name:"Crushed Coral",icon:"/gems/Item_953.png",usages:[{archetype:"Melee",slot:"Helm"}]},
 {name:"Crushed Topaz",icon:"/gems/Item_954.png",usages:[{archetype:"Melee",slot:"Gloves"},{archetype:"Caster",slot:"Gloves"}]},
 {name:"Crushed Black Marble",icon:"/gems/Item_956.png",usages:[{archetype:"Melee",slot:"Boots"}]},
 {name:"Crushed Flame Emerald",icon:"/gems/Item_962.png",usages:[{archetype:"Melee",slot:"Bracer"},{archetype:"Priest",slot:"Boots"}]},
 {name:"Black Marble",icon:"/gems/Item_956.png",usages:[{archetype:"Priest",slot:"Chest"}]},
 {name:"Chipped Onyx Sapphire",icon:"/gems/Item_965.png",usages:[{archetype:"Priest",slot:"Legs"}]},
 {name:"Jaundice Gem",icon:"/gems/Item_951.png",usages:[{archetype:"Priest",slot:"Arms"}]},
 {name:"Crushed Onyx Sapphire",icon:"/gems/Item_965.png",usages:[{archetype:"Priest",slot:"Helm"},{archetype:"Caster",slot:"Bracer"}]},
 {name:"Crushed Lava Ruby",icon:"/gems/Item_964.png",usages:[{archetype:"Priest",slot:"Gloves"}]},
 {name:"Crushed Opal",icon:"/gems/Item_959.png",usages:[{archetype:"Priest",slot:"Bracer"}]},
 {name:"Pristine Emerald",icon:"/gems/Item_958.png",usages:[{archetype:"Caster",slot:"Chest"}]},
 {name:"Nephrite",icon:"/gems/Item_952.png",usages:[{archetype:"Caster",slot:"Legs"}]},
 {name:"Flawed Topaz",icon:"/gems/Item_954.png",usages:[{archetype:"Caster",slot:"Arms"}]},
 {name:"Crushed Flame Opal",icon:"/gems/Item_960.png",usages:[{archetype:"Caster",slot:"Helm"}]},
 {name:"Crushed Jaundice Gem",icon:"/gems/Item_951.png",usages:[{archetype:"Caster",slot:"Boots"}]},
];

const VELIOUS_ARMOR_GEM_NAMES=new Set(VELIOUS_ARMOR_GEMS.map(gem=>gem.name.toLocaleLowerCase()));
export const isVeliousArmorGem=(itemName:string)=>VELIOUS_ARMOR_GEM_NAMES.has(itemName.trim().toLocaleLowerCase());
export const gemFor=(archetype:ArmorArchetype,slot:ArmorSlot)=>VELIOUS_ARMOR_GEMS.find(gem=>gem.usages.some(usage=>usage.archetype===archetype&&usage.slot===slot));
