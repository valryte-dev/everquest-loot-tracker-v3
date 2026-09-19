import catalogJson from "./item-particles.json";

export type ItemParticleDefinition={
 sprite:string;
 rate:number;
 scale:number;
 velocity:number;
 radius:number;
 lifespan:number;
 count:number;
 color:[number,number,number];
 movement:number;
 normal:[number,number,number];
 bones:string[];
};

export type ItemParticleRenderSettings={
 capacity:number;
 emitRate:number;
 displayScale:number;
 minSize:number;
 maxSize:number;
 sizeMultiplier:number;
 minLifeTime:number;
 maxLifeTime:number;
 minEmitPower:number;
 maxEmitPower:number;
 radius:number;
 updateSpeed:number;
 color:[number,number,number];
 direction:[number,number,number]|null;
};

const CATALOG=catalogJson as unknown as Record<string,ItemParticleDefinition[]>;

// Model aliases are copied from P99 Planner's paper-doll renderer. Resolve the
// model before both GLB loading and particle lookup so effects stay attached
// to the geometry that actually rendered.
const MODEL_ALIASES:Record<string,string>={
 IT10007:"IT7",IT10608:"IT7",IT10603:"IT18",IT10649:"IT85",
 IT10652:"IT85",IT10653:"IT82",IT10648:"IT110",IT10650:"IT119",
 IT10100:"IT103",IT10613:"IT103",IT10638:"IT24",IT10601:"IT15",
 IT11501:"IT137",IT11502:"IT21",IT10512:"IT68",IT10511:"IT68",
 IT10510:"IT68",IT10733:"IT29",IT10645:"IT27",IT10646:"IT28",
};

export function animatedItemModel(idFile:string){
 const normalized=idFile.trim().toUpperCase();
 return MODEL_ALIASES[normalized]||normalized;
}

export function itemParticleDefinitions(idFile:string){
 const model=animatedItemModel(idFile);
 return CATALOG[model.toLocaleLowerCase()]||[];
}

export function itemParticleStartDelay(idFile:string,emitterIndex:number,emitterCount:number,emitRate:number){
 if(animatedItemModel(idFile)!=="IT150"||emitterCount<2||emitRate<=0)return 0;
 return emitterIndex*(1000/emitRate/emitterCount);
}

export function itemParticleRenderSettings(
 definition:ItemParticleDefinition,
 idFile:string,
):ItemParticleRenderSettings{
 const model=animatedItemModel(idFile);
 // Nature Walker's Scimitar was visually calibrated against a captured
 // in-game/P99 render. Keep that approved lifecycle while every other item
 // follows the authoritative P99 normalization below.
 if(model==="IT150")return{
  capacity:12,emitRate:40,displayScale:.35,
  minSize:.005,maxSize:.0079,sizeMultiplier:.5,
  minLifeTime:.0675,maxLifeTime:.1125,
  minEmitPower:19.5,maxEmitPower:48.75,
  radius:.02,updateSpeed:.012,
  color:definition.color,direction:null,
 };

 const life=Math.min(definition.lifespan||750,2000)/1000;
 const size=Math.max(definition.scale||1,.05)*.5;
 const [x,y,z]=definition.normal||[0,0,0];
 const directed=definition.movement===3&&(x!==0||y!==0||z!==0);
 const length=directed?Math.hypot(x,y,z):1;
 const power=Math.max(Math.abs(definition.velocity||1),.1)*(directed?1:.15);
 return{
  capacity:Math.min(Math.max(definition.count||12,1),60),
  emitRate:Math.min(Math.max(Math.round(1000/Math.max(definition.rate||100,1)),1),200),
  displayScale:1,
  minSize:+(size*(directed?1:.7)).toFixed(4),
  maxSize:+(size*(directed?1:1.1)).toFixed(4),
  sizeMultiplier:1,
  minLifeTime:+(life*(directed?1:.6)).toFixed(4),
  maxLifeTime:+life.toFixed(4),
  minEmitPower:+(power*(directed?1:.3)).toFixed(4),
  maxEmitPower:+power.toFixed(4),
  radius:+Math.max(definition.radius||0,.02).toFixed(4),
  updateSpeed:.012,
  color:definition.color||[1,1,1],
  direction:directed?[
   +(-z/length).toFixed(4),
   +(y/length).toFixed(4),
   +(x/length).toFixed(4),
  ]:null,
 };
}
