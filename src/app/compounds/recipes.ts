import type {CompoundTemplate} from "./model";

const recipe=(name:string,components:string[]):CompoundTemplate=>({id:`builtin:${name.toLocaleLowerCase().replace(/[^a-z0-9]+/g,"-")}`,name,itemId:null,builtIn:true,components:components.map(itemName=>({itemId:null,itemName,required:1,valuePp:0}))});

export const BUILTIN_COMPOUND_TEMPLATES:CompoundTemplate[]=[
 recipe("Black Flower of Functionality",["A Black Squire","A White Squire","A Red Squire","A Blue Squire"]),
 recipe("Blue Flower of Functionality",["A Black Throne","A White Throne","A Red Throne","A Blue Throne"]),
 recipe("Green Flower of Functionality",["A Black Knight","A White Knight","A Red Knight","A Blue Knight"]),
 recipe("White Flower of Functionality",["A Black Crown","A White Crown","A Red Crown","A Blue Crown"]),
 recipe("Red Flower of Functionality",["A White Throne","A White Crown","A White Knight","A White Squire"]),
 recipe("Belt of Inconsistency",["A White Throne","A Blue Throne","A White Crown","A Blue Crown"]),
 recipe("Globe of Darkness",["A Black Throne","A Blue Throne","A Black Crown","A Blue Crown"]),
 recipe("Idiot Savant's Cap",["A Red Throne","A White Throne","A Red Crown","A White Crown"]),
 recipe("Very Rusty Dagger",["A Black Throne","A White Throne","A Black Crown","A White Crown"]),
 recipe("Buckler of Doom",["A Black Throne","A Black Crown","A Black Knight","A Black Squire"]),
 recipe("Cloak of Confusion",["A Blue Throne","A Blue Crown","A Blue Knight","A Blue Squire"]),
 recipe("Mask of Melodies",["A Red Throne","A Red Crown","A Red Knight","A Red Squire"]),
 recipe("Bracelet of the Twisted Mind",["A White Throne","A Blue Throne","A Blue Crown","A Blue Knight"]),
 recipe("Green Wristguard",["A White Throne","A Red Throne","A Red Crown","A Red Knight"]),
 recipe("Mushroom Bracelet",["A Black Throne","A Blue Throne","A Blue Crown","A Blue Knight"]),
 recipe("Onyx Wristbands",["A Blue Throne","A Red Throne","A Red Crown","A Red Knight"]),
 recipe("Shimmering Wristguard",["A Red Throne","A Blue Throne","A Blue Crown","A Blue Knight"]),
 recipe("Silver Wristguards",["A Black Throne","A Red Throne","A Red Crown","A Red Knight"]),
 recipe("Boots of Distraction",["A White Throne","A Blue Throne","A White Crown","A White Knight"]),
 recipe("Breastplate of Distraction",["A Red Throne","A Black Throne","A Black Crown","A Black Knight"]),
 recipe("Crown of Distraction",["A Red Throne","A Blue Throne","A Red Crown","A Blue Crown"]),
 recipe("Gloves of Distraction",["A White Throne","A Red Throne","A White Crown","A White Knight"]),
 recipe("Greaves of Distraction",["A Blue Throne","A Black Throne","A Black Crown","A Black Knight"]),
 recipe("Robe of Distraction",["A Black Throne","A Red Throne","A Black Crown","A Red Crown"]),
 recipe("Vambraces of Distraction",["A White Throne","A Black Crown","A Black Knight","A Black Squire"]),
 recipe("Wristguard of Distraction",["A White Throne","A Black Throne","A White Crown","A White Knight"]),
];

export const BUILTIN_COMPOUND_RECIPES:Record<string,string[]>=Object.fromEntries(BUILTIN_COMPOUND_TEMPLATES.map(template=>[template.name,template.components.flatMap(part=>Array(part.required).fill(part.itemName))]));
