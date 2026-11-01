import {renderToStaticMarkup} from "react-dom/server";
import {describe,expect,it} from "vitest";
import type {WardrobeCatalogItem} from "../../shared/contracts";
import {WardrobeQuickCompare} from "./WardrobeQuickCompare";
import {SLOT_BITS,type WardrobeCompareEntry} from "./model";

const item=(overrides:Partial<WardrobeCatalogItem>={}):WardrobeCatalogItem=>({id:1,name:"Test item",noDrop:false,slots:SLOT_BITS.head,classes:1,races:1,weight:1,ac:10,hp:20,mana:0,strength:0,stamina:0,agility:0,dexterity:0,intelligence:0,wisdom:0,charisma:0,magicResist:0,fireResist:0,coldResist:0,diseaseResist:0,poisonResist:0,attack:0,haste:0,manaRegen:0,damageShield:0,damage:0,delay:0,clickName:"",procName:"",wornName:"",focusName:"",setNames:[],...overrides});

describe("wardrobe quick compare",()=>{
 it("renders candidates, displaced-item deltas, priorities, and combined totals",()=>{
  const oldHelm=item({id:1,name:"Old Helm",ac:5,hp:10});
  const newHelm=item({id:2,name:"New Helm",ac:15,hp:25});
  const entries:WardrobeCompareEntry[]=[{id:"head:2",item:newHelm,slotKey:"head",priority:1,enabled:true,note:"Kael upgrade"}];
  const market=(value:WardrobeCatalogItem)=>({id:value.id,name:value.name,valuePp:value.id===2?1500:200,valueBasis:"30-day WTS",count30d:4,lastSeen:"",manual:false,source:"market"});
  const html=renderToStaticMarkup(<WardrobeQuickCompare entries={entries} equipped={{head:oldHelm}} market={market} onAdd={()=>{}} onChange={()=>{}} onApply={()=>{}}/>);
  expect(html).toContain("Quick Compare");
  expect(html).toContain("New Helm");
  expect(html).toContain("vs Old Helm");
  expect(html).toContain("AC +10");
  expect(html).toContain("Kael upgrade");
  expect(html).toContain("Combined totals");
  expect(html).toContain("High");
  expect(html).toContain("Compared upgrades");
  expect(html).toContain("1,500 pp");
  expect(html).toContain("Projected outfit value");
 });
});