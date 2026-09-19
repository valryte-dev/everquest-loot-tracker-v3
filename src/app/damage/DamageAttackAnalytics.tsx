import type {DamageAttackTypeMetric} from "../../shared/contracts";

const number=(value:number)=>Math.round(value).toLocaleString();
const label=(value:string)=>value.replace(/_/g," ").replace(/\b\w/g,letter=>letter.toUpperCase());
const isMelee=(metric:DamageAttackTypeMetric)=>metric.damageType.toLowerCase()==="melee";

export function DamageAttackAnalytics({metrics}:{metrics:DamageAttackTypeMetric[]}){
 const melee=metrics.filter(isMelee).sort((a,b)=>b.totalDamage-a.totalDamage).slice(0,10);
 const fingerprint=[...metrics].filter(row=>row.hitCount>0).sort((a,b)=>b.totalDamage-a.totalDamage).slice(0,14);
 return <>
  <article className="card damage-technique-card"><header><div><h2>Melee technique breakdown</h2><p>Which attack types actually carry your physical damage.</p></div></header><TechniqueRows rows={melee}/></article>
  <article className="card damage-fingerprint-card"><header><div><h2>Combat fingerprint</h2><p>Frequency vs impact; bubble size represents total contribution.</p></div></header><Fingerprint rows={fingerprint}/></article>
 </>;
}

function TechniqueRows({rows}:{rows:DamageAttackTypeMetric[]}){
 if(!rows.length)return <div className="damage-empty">Attack techniques will appear after combat is recorded.</div>;
 const total=rows.reduce((sum,row)=>sum+row.totalDamage,0),max=Math.max(...rows.map(row=>row.totalDamage),1);
 return <div className="damage-techniques">{rows.map(row=><div key={`${row.attack}-${row.damageType}`}><span>{label(row.attack)}<small>{number(row.hitCount)} hits / {number(row.totalDamage/row.hitCount)} avg / {number(row.maxHit)} max</small></span><i><em style={{width:`${row.totalDamage/max*100}%`}}/></i><strong>{number(row.totalDamage)}<small>{Math.round(row.totalDamage/Math.max(1,total)*100)}%</small></strong></div>)}</div>;
}

function Fingerprint({rows}:{rows:DamageAttackTypeMetric[]}){
 if(!rows.length)return <div className="damage-empty">Your combat fingerprint needs recorded damage events.</div>;
 const values=rows.map(row=>({row,average:row.totalDamage/row.hitCount}));
 const maxHits=Math.max(...values.map(value=>value.row.hitCount),1),maxAverage=Math.max(...values.map(value=>value.average),1),maxTotal=Math.max(...values.map(value=>value.row.totalDamage),1);
 return <div className="damage-fingerprint"><svg viewBox="0 0 720 330" role="img" aria-label="Attack frequency and impact bubble chart">
  <rect x="58" y="24" width="632" height="250" rx="12"/><line x1="374" y1="24" x2="374" y2="274"/><line x1="58" y1="149" x2="690" y2="149"/>
  <text className="quadrant" x="72" y="43">HEAVY HITTERS</text><text className="quadrant" x="676" y="43" textAnchor="end">HEAVY + FREQUENT</text><text className="quadrant" x="72" y="264">LIGHT TOUCH</text><text className="quadrant" x="676" y="264" textAnchor="end">STEADY PRESSURE</text>
  {values.map(({row,average})=>{const x=82+row.hitCount/maxHits*580,y=250-average/maxAverage*196,r=7+Math.sqrt(row.totalDamage/maxTotal)*18;return <g key={`${row.attack}-${row.damageType}`} className={isMelee(row)?"melee":"spell"}><circle cx={x} cy={y} r={r}/><text x={x} y={y+r+13} textAnchor="middle">{label(row.attack)}</text><title>{`${label(row.attack)}: ${number(row.hitCount)} hits, ${number(average)} average, ${number(row.totalDamage)} total`}</title></g>})}
  <text className="axis" x="374" y="314" textAnchor="middle">Hit frequency -&gt;</text><text className="axis impact" transform="translate(18 149) rotate(-90)" textAnchor="middle">Average impact -&gt;</text>
 </svg><div className="damage-fingerprint-key"><span><i className="melee"/>Melee technique</span><span><i className="spell"/>Spell / proc / DoT</span></div></div>;
}
