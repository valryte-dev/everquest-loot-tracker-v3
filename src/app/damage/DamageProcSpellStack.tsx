import type {DamageEncounter} from "../../shared/contracts";
import {DamageProcEvidence} from "./DamageProcEvidence";
import {TrackedSpellsPanel} from "./DamageSpellActivity";

export function DamageProcSpellStack({row}:{row:DamageEncounter}){
 return <div className="damage-proc-spell-stack">
  <DamageProcEvidence encounterId={row.id} metrics={row.spellMetrics||[]}/>
  <TrackedSpellsPanel row={row}/>
 </div>;
}
