import {useMemo, useState, type ReactNode} from "react";
import {isVeliousArmorGem} from "./gems/catalog";

export type Column<T> = {key:string; label:string; value:(row:T)=>unknown; render?:(row:T)=>ReactNode; className?:string};
type Props<T> = {rows:T[]; columns:Column<T>[]; rowKey:(row:T)=>string|number; actions?:(row:T)=>ReactNode; selected?:Set<string|number>; onSelected?:(next:Set<string|number>)=>void; empty?:string; dimInsteadOfHide?:boolean; rowClass?:(row:T)=>string};

export function DataTable<T>({rows, columns, rowKey, actions, selected, onSelected, empty="No records yet.", dimInsteadOfHide=false, rowClass}:Props<T>) {
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
  return <div className="data-grid"><div className="grid-tools"><label className="search"><span>⌕</span><input value={filter} onChange={event=>setFilter(event.target.value)} placeholder="Filter this list…"/><button disabled={!filter} onClick={()=>setFilter("")} aria-label="Clear filter">×</button></label><span>{selectable.length} of {rows.length}{shown.length>visible.length?` · showing first ${visible.length}`:""}</span></div><div className="table-scroll"><table><thead><tr>{columns.map(column=><th key={column.key} onClick={()=>toggleSort(column.key)} className={column.className}>{column.label}<span className="sort">{sort===column.key?(desc?"↓":"↑"):""}</span></th>)}{actions&&<th>Actions</th>}{onSelected&&<th className="select-col">Select <input type="checkbox" checked={all} onChange={()=>onSelected(new Set(all?[]:selectable.map(rowKey)))}/></th>}</tr></thead><tbody>{visible.map(row=><tr key={rowKey(row)} className={[matches(row)?"":"filter-dimmed",specialGem(row)?"special-gem-row":"",rowClass?.(row)||""].filter(Boolean).join(" ")}>{columns.map(column=><td key={column.key} className={column.className}>{column.render?column.render(row):String(column.value(row)??"—")}</td>)}{actions&&<td className="row-actions">{actions(row)}</td>}{onSelected&&<td className="select-col"><input type="checkbox" checked={selected?.has(rowKey(row))??false} onChange={()=>{const next=new Set(selected);const key=rowKey(row);next.has(key)?next.delete(key):next.add(key);onSelected(next)}}/></td>}</tr>)}{!shown.length&&<tr><td className="empty-cell" colSpan={columns.length+(actions?1:0)+(onSelected?1:0)}>{empty}</td></tr>}</tbody></table></div></div>;
}

export function Modal({title,children,onClose,footer}:{title:string;children:ReactNode;onClose:()=>void;footer?:ReactNode}){return <div className="modal-backdrop" onMouseDown={event=>event.target===event.currentTarget&&onClose()}><section className="modal" role="dialog" aria-modal="true"><header><h2>{title}</h2><button onClick={onClose} aria-label="Close">×</button></header><div className="modal-body">{children}</div>{footer&&<footer>{footer}</footer>}</section></div>}
export const Field=({label,children}:{label:string;children:ReactNode})=><label className="field"><span>{label}</span>{children}</label>;
export const money=(value?:number|null)=>value==null?"—":`${value.toLocaleString()} pp`;
export const when=(value:string)=>{const date=new Date(value);return Number.isNaN(date.valueOf())?value:date.toLocaleString()};
