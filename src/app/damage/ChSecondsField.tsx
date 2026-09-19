interface ChSecondsFieldProps {
 label:string;
 ariaLabel:string;
 value:string;
 min:number;
 max:number;
 step:number;
 placeholder?:string;
 compact?:boolean;
 className?:string;
 onChange:(value:string)=>void;
 onCommit?:()=>void;
}

export function ChSecondsField({label,ariaLabel,value,min,max,step,placeholder,compact=false,className="",onChange,onCommit}:ChSecondsFieldProps){
 return <label className={`ch-seconds-field${compact?" compact":""}${className?" "+className:""}`}>
  <span>{label}</span>
  <span className="ch-seconds-input">
   <input aria-label={ariaLabel} type="number" min={min} max={max} step={step} inputMode="decimal" value={value} placeholder={placeholder} onChange={event=>onChange(event.target.value)} onBlur={onCommit}/>
   <b aria-hidden="true">s</b>
  </span>
 </label>;
}
