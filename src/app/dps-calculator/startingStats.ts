import {GENERATED_STARTING_STATS} from "./startingStats.generated";
import type {DpsClass} from "./model";

export interface StartingStats {
 race:string;
 characterClass:DpsClass;
 strength:number;
 stamina:number;
 agility:number;
 dexterity:number;
 wisdom:number;
 intelligence:number;
 charisma:number;
 bonusPoints:number;
}

const key=(race:string,characterClass:string)=>race.trim().toLocaleLowerCase()+"|"+characterClass.trim().toLocaleLowerCase();
const byCombination=new Map(GENERATED_STARTING_STATS.map(row=>[key(row.race,row.characterClass),row]));
export const DPS_RACES=[...new Set(GENERATED_STARTING_STATS.map(row=>row.race))].sort((left,right)=>left.localeCompare(right));
export const startingStatsFor=(race:string,characterClass:DpsClass)=>byCombination.get(key(race,characterClass));
export const racesForDpsClass=(characterClass:DpsClass)=>DPS_RACES.filter(race=>Boolean(startingStatsFor(race,characterClass)));
export const classesForDpsRace=(race:string,classes:readonly DpsClass[])=>classes.filter(characterClass=>Boolean(startingStatsFor(race,characterClass)));
