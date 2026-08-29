import {useMemo, useState, type ButtonHTMLAttributes, type ReactNode} from "react";
import {isVeliousArmorGem} from "./gems/catalog";

export type Column<T> = {key:string; label:string; value:(row:T)=>unknown; render?:(row:T)=>ReactNode; className?:string};
export type IconName="add"|"bookmark"|"check"|"clipboard"|"coin"|"download"|"edit"|"external"|"flame"|"pause"|"play"|"refresh"|"save"|"split"|"trash";
const ICON_PATHS:Record<IconName,ReactNode>={
  add:<><path d="M12 5v14M5 12h14"/></>,
  bookmark:<path d="M6 4.75A1.75 1.75 0 0 1 7.75 3h8.5A1.75 1.75 0 0 1 18 4.75V21l-6-3.5L6 21Z"/>,
  check:<path d="m5 12 4 4L19 6"/>,
  clipboard:<><rect x="5" y="4" width="14" height="17" rx="2"/><path d="M9 4.5V3h6v1.5M9 9h6M9 13h6M9 17h4"/></>,
  coin:<><circle cx="12" cy="12" r="9"/><path d="M15.5 8.5c-.8-.7-1.8-1-3-1-1.7 0-3 .8-3 2s1.1 1.8 3 2.2 3 1 3 2.3-1.3 2.5-3.2 2.5c-1.2 0-2.4-.4-3.3-1.2M12 5.5v13"/></>,
  download:<><path d="M12 3v12M7 10l5 5 5-5"/><path d="M5 21h14"/></>,
  edit:<><path d="M4 20h4l11-11-4-4L4 16v4Z"/><path d="m13.5 6.5 4 4"/></>,
  external:<><path d="M14 4h6v6M20 4l-9 9"/><path d="M18 13v6a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h6"/></>,
  flame:<path d="M13 22c4-1 7-4 7-8 0-3-1.5-5.5-4-8 .2 3-1 4.5-2.5 5.5C14 7 11 4 8 2c.3 3-1.5 5.5-3 7.5S3 13 4 16c1.2 3.5 4.4 5.5 9 6Z"/>,
  pause:<><path d="M8 5v14M16 5v14"/></>,
  play:<path d="m8 5 11 7-11 7Z"/>,
  refresh:<><path d="M20 7v5h-5"/><path d="M19 12a7 7 0 1 0-2 5"/></>,
  save:<><path d="M5 3h12l3 3v15H4V4a1 1 0 0 1 1-1Z"/><path d="M8 3v6h8V3M8 21v-7h8v7"/></>,
  split:<><circle cx="8" cy="8" r="3"/><circle cx="17" cy="9" r="2.5"/><path d="M3 20c.5-4 2-6 5-6s4.5 2 5 6M14 15c3-1 5 .7 6 4"/></>,
  trash:<><path d="M4 7h16M9 7V4h6v3M7 7l1 14h8l1-14M10 11v6M14 11v6"/></>
};

export function IconButton({icon,label,className="",...props}:{icon:IconName;label:string}&ButtonHTMLAttributes<HTMLButtonElement>){return <button {...props} className={`icon-button ${className}`.trim()} aria-label={label} title={label}><svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">{ICON_PATHS[icon]}</svg></button>}

type Props<T> = {rows:T[]; columns:Column<T>[]; rowKey:(row:T)=>string|number; actions?:(row:T)=>ReactNode; selected?:Set<string|number>; onSelected?:(next:Set<string|number>)=>void; empty?:string; dimInsteadOfHide?:boolean; rowClass?:(row:T)=>string; hideTools?:boolean};

export function DataTable<T>({rows, columns, rowKey, actions, selected, onSelected, empty="No records yet.", dimInsteadOfHide=false, rowClass, hideTools=false}:Props<T>) {
  const [filter,setFilter]=useState("");
  const defaultSort=columns.some(column=>column.key==="time")?"time":columns[0]?.key??"";
  const [sort,setSort]=useState(defaultSort);
  const [desc,setDesc]=useState(defaultSort==="time");
  const matches=(row:T)=>!filter.trim()||columns.some(column=>String(column.value(row)??"").toLowerCase().includes(filter.trim().toLowerCase()));
  const shown=useMemo(()=>{const column=columns.find(item=>item.key===sort);return rows.filter(row=>dimInsteadOfHide||matches(row)).sort((a,b)=>{const av=column?.value(a)??"",bv=column?.value(b)??"";const comparison=typeof av==="number"&&typeof bv==="number"?av-bv:String(av).localeCompare(String(bv),undefined,{numeric:true,sensitivity:"base"});return desc?-comparison:comparison})},[rows,filter,sort,desc,columns,dimInsteadOfHide]);
  const toggleSort=(key:string)=>{if(sort===key)setDesc(value=>!value);else{setSort(key);setDesc(false)}};
  const selectable=shown.filter(matches);
  const visible=shown.slice(0,250);
  const all=selectable.length>0&&selectable.every(row=>selected?.has(rowKey(row)));
  const specialGem=(row:T)=>{const itemName=(row as {itemName?:unknown})?.itemName;return typeof itemName==="string"&&isVeliousArmorGem(itemName)};
  return <div className="data-grid">{!hideTools&&<div className="grid-tools"><label className="search"><span>⌕</span><input value={filter} onChange={event=>setFilter(event.target.value)} placeholder="Filter this list…"/><button disabled={!filter} onClick={()=>setFilter("")} aria-label="Clear filter">×</button></label><span>{selectable.length} of {rows.length}{shown.length>visible.length?` · showing first ${visible.length}`:""}</span></div>}<div className="table-scroll"><table><thead><tr>{columns.map(column=><th key={column.key} onClick={()=>toggleSort(column.key)} className={column.className}>{column.label}<span className="sort">{sort===column.key?(desc?"↓":"↑"):""}</span></th>)}{actions&&<th>Actions</th>}{onSelected&&<th className="select-col">Select <input type="checkbox" checked={all} onChange={()=>onSelected(new Set(all?[]:selectable.map(rowKey)))}/></th>}</tr></thead><tbody>{visible.map(row=><tr key={rowKey(row)} className={[matches(row)?"":"filter-dimmed",specialGem(row)?"special-gem-row":"",rowClass?.(row)||""].filter(Boolean).join(" ")}>{columns.map(column=><td key={column.key} className={column.className}>{column.render?column.render(row):String(column.value(row)??"—")}</td>)}{actions&&<td className="row-actions">{actions(row)}</td>}{onSelected&&<td className="select-col"><input type="checkbox" checked={selected?.has(rowKey(row))??false} onChange={()=>{const next=new Set(selected);const key=rowKey(row);next.has(key)?next.delete(key):next.add(key);onSelected(next)}}/></td>}</tr>)}{!shown.length&&<tr><td className="empty-cell" colSpan={columns.length+(actions?1:0)+(onSelected?1:0)}>{empty}</td></tr>}</tbody></table></div></div>;
}

export function Modal({title,children,onClose,footer}:{title:string;children:ReactNode;onClose:()=>void;footer?:ReactNode}){return <div className="modal-backdrop" onMouseDown={event=>event.target===event.currentTarget&&onClose()}><section className="modal" role="dialog" aria-modal="true"><header><h2>{title}</h2><button onClick={onClose} aria-label="Close">×</button></header><div className="modal-body">{children}</div>{footer&&<footer>{footer}</footer>}</section></div>}
export const Field=({label,children}:{label:string;children:ReactNode})=><label className="field"><span>{label}</span>{children}</label>;
export const money=(value?:number|null)=>value==null?"—":`${value.toLocaleString()} pp`;
export const when=(value:string)=>{const date=new Date(value);return Number.isNaN(date.valueOf())?value:date.toLocaleString()};
