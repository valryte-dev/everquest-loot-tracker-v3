import type {CharacterProfile} from "../../shared/contracts";

export function filterRosterCharacters(characters:string[],profiles:CharacterProfile[],nameFilter:string,classFilter:string){
 const normalizedName=nameFilter.trim().toLocaleLowerCase();
 const profileByCharacter=new Map(profiles.map(profile=>[profile.character.toLocaleLowerCase(),profile]));
 return characters.filter(character=>{
  if(normalizedName&&!character.toLocaleLowerCase().includes(normalizedName))return false;
  if(!classFilter)return true;
  const classCode=profileByCharacter.get(character.toLocaleLowerCase())?.classCode||"";
  return classFilter==="unknown"?!classCode:classCode===classFilter;
 });
}