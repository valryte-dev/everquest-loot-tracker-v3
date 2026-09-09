import type {ClericChainEncounter} from "./chReplay";

let pendingEncounter:ClericChainEncounter|null=null;

export function openEncounterInChLab(encounter:ClericChainEncounter){
 pendingEncounter=encounter;
 window.location.hash="#/ch-lab";
}

export function takePendingChLabEncounter(){
 const encounter=pendingEncounter;
 pendingEncounter=null;
 return encounter;
}