import {useCallback,useEffect,useMemo,useState} from "react";
import type {AppSnapshot,FriendsSyncSnapshot} from "../../shared/contracts";
import {applyFriendsSync,getFriendsSyncSnapshot,updateFriendsList} from "../../shared/backend";
import {DataTable,Field,type Column} from "../ui";
import "./friends-sync.css";

type Notice={title:string;detail:string};

export function FriendsSyncPage({data}:{data:AppSnapshot}){
 const[state,setState]=useState<FriendsSyncSnapshot|null>(null);
 const[source,setSource]=useState("");
 const[draftFriends,setDraftFriends]=useState<string[]>([]);
 const[newFriend,setNewFriend]=useState("");
 const[dirty,setDirty]=useState(false);
 const[loading,setLoading]=useState(true);
 const[busy,setBusy]=useState<""|"save"|"sync">("");
 const[error,setError]=useState("");
 const[notice,setNotice]=useState<Notice|null>(null);
 const load=useCallback(async()=>{
  setLoading(true);setError("");
  try{
   const value=await getFriendsSyncSnapshot();
   setState(value);
   setSource(current=>{
    if(value.characters.some(character=>character.fileName===current&&character.hasFriendsSection))return current;
    const active=value.characters.find(character=>character.character.toLowerCase()===data.settings.active_character?.toLowerCase()&&character.hasFriendsSection);
    return active?.fileName||value.characters.find(character=>character.hasFriendsSection)?.fileName||"";
   });
  }catch(reason){setError(String(reason))}finally{setLoading(false)}
 },[data.settings.active_character]);
 useEffect(()=>{void load()},[load]);
 const selected=state?.characters.find(character=>character.fileName===source);
 const selectedSignature=selected?.friends.join("\u001f")||"";
 useEffect(()=>{setDraftFriends(selected?.friends||[]);setNewFriend("");setDirty(false)},[selected?.fileName,selectedSignature]);
 const targets=useMemo(()=>state?.characters.filter(character=>character.fileName!==source)||[],[state,source]);
 const columns:Column<NonNullable<typeof state>["characters"][number]>[]=[
  {key:"character",label:"Character",value:row=>row.character},
  {key:"server",label:"Server",value:row=>row.server},
  {key:"friends",label:"Current friends",value:row=>row.friends.length},
  {key:"file",label:"INI file",value:row=>row.fileName},
 ];
 const addFriend=()=>{
  const name=newFriend.trim();
  if(!name)return;
  if(!/^[A-Za-z]+$/.test(name)){setError("EverQuest character names must contain letters only.");return}
  if(name.length>64){setError("EverQuest character names must be 64 letters or fewer.");return}
  if(draftFriends.some(friend=>friend.toLowerCase()===name.toLowerCase())){setError(name+" is already in this Friends list.");return}
  if(draftFriends.length>=100){setError("This Friends section already uses all 100 slots.");return}
  setDraftFriends(values=>[...values,name]);setNewFriend("");setDirty(true);setError("");setNotice(null);
 };
 const removeFriend=(name:string)=>{setDraftFriends(values=>values.filter(friend=>friend!==name));setDirty(true);setNotice(null)};
 const chooseSource=(fileName:string)=>{
  if(dirty&&!window.confirm("Discard unsaved Friends-list changes?"))return;
  setSource(fileName);setNotice(null);setError("");
 };
 const rescan=()=>{
  if(dirty&&!window.confirm("Discard unsaved Friends-list changes and rescan the INI files?"))return;
  void load();
 };
 const save=async()=>{
  if(!selected||!dirty)return;
  if(!window.confirm("Close EverQuest before continuing. Save "+draftFriends.length+" friends to "+selected.character+"? The original INI will be backed up first."))return;
  setBusy("save");setError("");setNotice(null);
  try{
   const value=await updateFriendsList(selected.fileName,draftFriends);
   setNotice({title:"Saved "+value.friendCount+" friends to "+value.sourceCharacter+".",detail:"Backup: "+value.backupPath});
   setDirty(false);await load();
  }catch(reason){setError(String(reason))}finally{setBusy("")}
 };
 const apply=async()=>{
  if(!selected||!targets.length||dirty)return;
  const warning="Close EverQuest before continuing. Apply "+selected.friends.length+" friends from "+selected.character+" to "+targets.length+" other character INI files? Every target will be backed up first.";
  if(!window.confirm(warning))return;
  setBusy("sync");setError("");setNotice(null);
  try{
   const value=await applyFriendsSync(selected.fileName);
   setNotice({title:"Synchronized "+value.friendCount+" friends to "+value.appliedCount+" characters.",detail:"Backup: "+value.backupDirectory});
   await load();
  }catch(reason){setError(String(reason))}finally{setBusy("")}
 };
 if(loading&&!state)return <div className="friends-sync-state"><span className="spinner"/><p>Scanning character INI files...</p></div>;
 if(error&&!state)return <div className="alert"><strong>Friends sync unavailable</strong><span>{error}</span><button onClick={()=>void load()}>Retry</button></div>;
 return <div className="friends-sync-page">
  <section className="card friends-sync-source">
   <header><div><h2>Friends list editor</h2><p>Select a character, add or remove friends, save it, then optionally apply that list to every character.</p></div><button onClick={rescan} disabled={Boolean(busy)}>Rescan INIs</button></header>
   <div className="friends-sync-controls">
    <Field label="Source character"><select value={source} disabled={Boolean(busy)} onChange={event=>chooseSource(event.target.value)}>
     {(state?.characters||[]).map(character=><option key={character.fileName} value={character.fileName} disabled={!character.hasFriendsSection}>{character.character} · {character.server}{character.hasFriendsSection?"":" · no Friends section"}</option>)}
    </select></Field>
    <dl className="friends-sync-summary"><div><dt>EverQuest folder</dt><dd>{state?.directory||"Not configured"}</dd></div><div><dt>Friends</dt><dd>{draftFriends.length} / 100{dirty?" · unsaved":""}</dd></div><div><dt>Sync targets</dt><dd>{targets.length}</dd></div></dl>
   </div>
   {selected&&<form className="friends-sync-add" onSubmit={event=>{event.preventDefault();addFriend()}}><label><span>Add friend</span><input value={newFriend} onChange={event=>setNewFriend(event.target.value)} placeholder="Character name" disabled={Boolean(busy)}/></label><button type="submit" disabled={!newFriend.trim()||Boolean(busy)||draftFriends.length>=100}>Add</button></form>}
   {selected?<div className="friends-sync-preview" aria-label={"Friends from "+selected.character}>{draftFriends.length?draftFriends.map(friend=><span key={friend.toLowerCase()}>{friend}<button type="button" onClick={()=>removeFriend(friend)} disabled={Boolean(busy)} aria-label={"Remove "+friend} title={"Remove "+friend}>×</button></span>):<p>This list is empty. Saving it will clear all populated friend slots for this character.</p>}</div>:<p className="friends-sync-empty">No character with a [Friends] section was found.</p>}
   <div className="friends-sync-action"><div><strong>{dirty?"Unsaved changes":"Safe synchronization"}</strong><p>{dirty?"Save this source list before applying it to other characters.":"Only [Friends] is changed. Ignored players, Socials, and every other INI section remain untouched."}</p></div><div className="button-row"><button onClick={save} disabled={!selected||!dirty||Boolean(busy)}>{busy==="save"?"Saving...":"Save source list"}</button><button className="primary" onClick={apply} disabled={!selected||!targets.length||dirty||Boolean(busy)}>{busy==="sync"?"Synchronizing...":dirty?"Save before syncing":"Apply to all "+targets.length+" characters"}</button></div></div>
  </section>
  {error&&<div className="alert"><strong>Friends operation failed</strong><span>{error}</span><button onClick={()=>setError("")}>Dismiss</button></div>}
  {notice&&<div className="friends-sync-success"><strong>{notice.title}</strong><span>{notice.detail}</span></div>}
  <section className="card"><header><div><h2>Target characters</h2><p>Every discovered character INI except the selected source will receive the saved source list.</p></div></header><DataTable rows={targets} columns={columns} rowKey={row=>row.fileName} empty="No other character INI files were found."/></section>
 </div>;
}
