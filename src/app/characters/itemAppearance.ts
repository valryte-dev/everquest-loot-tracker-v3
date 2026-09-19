export type ItemRotation={x:number;y:number;z:number};

type ItemAppearance={
 rotation?:ItemRotation;
 animationSpeedScale?:number;
};

// The hosted GLBs omit a small amount of source-WLD appearance metadata.
// Keep those corrections in one auditable catalog rather than scattering
// item-name checks through the renderer.
const ITEM_APPEARANCE:Record<string,ItemAppearance>={
 IT150:{
  // IT150's emitter points are direct children of the root and its visually
  // verified path uses flattened geometry with this vertical correction.
  rotation:{x:Math.PI,y:0,z:0},
 },
 IT157:{
  // The exported serpent motion is visually quicker than the in-game idle.
  animationSpeedScale:.75,
 },
};

export function itemAppearance(idFile:string|undefined):ItemAppearance|undefined{
 return idFile?ITEM_APPEARANCE[idFile.trim().toUpperCase()]:undefined;
}

export function eqShader(metadata:unknown):number|undefined{
 const value=metadata as {gltf?:{extras?:{eqShader?:unknown}};extras?:{eqShader?:unknown};eqShader?:unknown}|null;
 const shader=value?.gltf?.extras?.eqShader??value?.extras?.eqShader??value?.eqShader;
 return typeof shader==="number"?shader:undefined;
}

export function isAdditiveEqShader(metadata:unknown){
 const shader=eqShader(metadata);
 return shader===4||shader===5||shader===9;
}
