export interface CharacterModelProfile {race:string;gender:"m"|"f";classCode:string}

export const CHARACTER_RACES=[
 {code:"ba",name:"Barbarian"},{code:"da",name:"Dark Elf"},{code:"dw",name:"Dwarf"},
 {code:"el",name:"Wood Elf"},{code:"er",name:"Erudite"},{code:"gn",name:"Gnome"},
 {code:"ha",name:"Half Elf"},{code:"hi",name:"High Elf"},{code:"ho",name:"Halfling"},
 {code:"hu",name:"Human"},{code:"ik",name:"Iksar"},{code:"og",name:"Ogre"},
 {code:"tr",name:"Troll"},
] as const;

export const CHARACTER_CLASSES=[
 {code:"",name:"Unknown"},{code:"war",name:"Warrior"},{code:"clr",name:"Cleric"},
 {code:"pal",name:"Paladin"},{code:"rng",name:"Ranger"},{code:"shd",name:"Shadow Knight"},
 {code:"dru",name:"Druid"},{code:"mnk",name:"Monk"},{code:"brd",name:"Bard"},
 {code:"rog",name:"Rogue"},{code:"shm",name:"Shaman"},{code:"nec",name:"Necromancer"},
 {code:"wiz",name:"Wizard"},{code:"mag",name:"Magician"},{code:"enc",name:"Enchanter"},
] as const;

// Project 1999's classic-through-Velious character creation matrix.
const RACE_CLASS_CODES:Record<string,ReadonlySet<string>>={
 ba:new Set(["rog","shm","war"]),
 da:new Set(["clr","enc","mag","nec","rog","shd","war","wiz"]),
 dw:new Set(["clr","pal","rog","war"]),
 el:new Set(["brd","dru","rng","rog","war"]),
 er:new Set(["clr","enc","mag","nec","pal","shd","wiz"]),
 gn:new Set(["clr","enc","mag","nec","rog","war","wiz"]),
 ha:new Set(["brd","dru","pal","rng","rog","war"]),
 hi:new Set(["clr","enc","mag","pal","wiz"]),
 ho:new Set(["clr","dru","rog","war"]),
 hu:new Set(["brd","clr","dru","enc","mag","mnk","nec","pal","rng","rog","shd","war","wiz"]),
 ik:new Set(["mnk","nec","shd","shm","war"]),
 og:new Set(["shd","shm","war"]),
 tr:new Set(["shd","shm","war"]),
};

export function isCharacterRaceClassCompatible(race:string,classCode:string){
 return !classCode||Boolean(RACE_CLASS_CODES[race]?.has(classCode));
}

export function characterClassesForRace(race:string){
 const allowed=RACE_CLASS_CODES[race];
 return CHARACTER_CLASSES.filter(entry=>!entry.code||allowed?.has(entry.code));
}

export function characterRacesForClass(classCode:string){
 return !classCode?[...CHARACTER_RACES]:CHARACTER_RACES.filter(race=>RACE_CLASS_CODES[race.code]?.has(classCode));
}

export function normalizeCharacterModelProfile(profile:CharacterModelProfile):CharacterModelProfile{
 const race=CHARACTER_RACES.some(entry=>entry.code===profile.race)?profile.race:"hu";
 const gender=profile.gender==="f"?"f":"m";
 const knownClass=CHARACTER_CLASSES.some(entry=>entry.code===profile.classCode)?profile.classCode:"";
 return {race,gender,classCode:isCharacterRaceClassCompatible(race,knownClass)?knownClass:""};
}

export function parseCharacterModelProfile(value?:string):CharacterModelProfile{
 try{return normalizeCharacterModelProfile(JSON.parse(value||"{}"))}
 catch{return {race:"hu",gender:"m",classCode:""}}
}
