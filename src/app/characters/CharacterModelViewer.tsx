import {useEffect,useRef,useState} from "react";
import {getModelPackStatus} from "../../shared/backend";
import type {InventoryItem} from "../../shared/contracts";
import type {CharacterModelProfile} from "./characterProfile";
import {isAdditiveEqShader,itemAppearance} from "./itemAppearance";
import {itemParticleDefinitions,itemParticleRenderSettings,itemParticleStartDelay} from "./itemParticleCatalog";

const REMOTE_MODEL_ROOT="https://p99planner.com/models/";
const LOCAL_MODEL_ROOT="http://127.0.0.1:8765/model-assets/";
export const DEFAULT_MODEL_RADIUS=11;
export const DEFAULT_MODEL_ALPHA=0;
export const MODEL_VERTICAL_OFFSET=.13;
// Match Babylon/EQSage's default alpha pipeline. Disabling premultiplied
// alpha makes the translucent planes used by animated shields and weapon
// effects render as visible quads instead of compositing their alpha channel.
export const MODEL_ENGINE_OPTIONS={preserveDrawingBuffer:false,stencil:false,premultipliedAlpha:true} as const;

const ROBE_MODEL_CODES=new Set(["daf","dam","erf","erm","gnf","gnm","hif","him","huf","hum","ikf","ikm"]);

export const modelRoots=(localReady:boolean)=>localReady?[LOCAL_MODEL_ROOT,REMOTE_MODEL_ROOT]:[REMOTE_MODEL_ROOT];

function visualSlot(location:string){
 const value=location.toLocaleLowerCase().replace(/[^a-z0-9]/g,"");
 if(value==="primary"||value==="secondary"||value==="head"||value==="chest"||value==="arms"||value==="hands"||value==="legs"||value==="feet")return value;
 if(/wrist/.test(value))return "wrist";
 return "";
}

export function visualEquipment(items:InventoryItem[]){
 const equipped=new Map<string,InventoryItem>();
 for(const item of items){
  const slot=visualSlot(item.location);
  if(!slot)continue;
  // EQ exports paired wrists as identical "Wrist" rows, left first.
  if(slot==="wrist"&&equipped.has(slot))continue;
  equipped.set(slot,item);
 }
 return equipped;
}

export function armorTextureName(materialName:string,material:number){
 const robe=/^clk\d{2}(\d{2})$/i.exec(materialName);
 if(robe&&material>=10&&material<=16)return `clk${String(material-6).padStart(2,"0")}${robe[1]}.png`;
 const match=/^([a-z]{3})(ch|ua|fa|hn|lg|ft)(\d{2})(\d+)$/i.exec(materialName);
 return match&&material>0&&material<100?`${match[1]}${match[2]}${String(material).padStart(2,"0")}${match[4]}.png`:undefined;
}

export function armorMaterial(item:InventoryItem|undefined,_modelCode=""){
 const material=item?.material||0;
 // The P99 item export uses material 7 for the Iksar scale family, while the
 // classic EQSage character assets expose that appearance as plate (3).
 return material===7?3:material;
}

export function replaceableArmorRegion(materialName:string,modelCode:string){
 // The fourth Iksar leg material is the character's tail, not an armor panel.
 // Re-skinning it with a leg armor texture removes the native tail markings.
 return !(/^ik[fm]lg\d{2}04$/i.test(materialName)&&/^ik[fm]/i.test(modelCode));
}

export async function firstImageAssetUrl(
 urls:string[],
 request:typeof fetch=fetch,
){
 for(const url of urls){
  try{
   const response=await request(url,{method:"HEAD"});
   if(response.ok&&(response.headers.get("content-type")||"").toLocaleLowerCase().startsWith("image/"))return url;
  }catch{/* Try the next configured model source. */}
 }
 return undefined;
}

export function robeModelCode(modelCode:string,chestMaterial:number){
 return chestMaterial>=10&&chestMaterial<=16&&ROBE_MODEL_CODES.has(modelCode)?`${modelCode}01`:modelCode;
}

export function isRobeHeadMaterial(materialName:string,bodyModelCode:string){
 // EQSage's secondary-head assets include a race-specific CLK material for
 // the hood/cowl (for example CLKERM06). It belongs to the robe's chest
 // appearance, not to the equipped helm or the character's face.
 return bodyModelCode.endsWith("01")&&/^clk[a-z]{3}\d{2}$/i.test(materialName);
}

export function materialAnimation(metadata:unknown){
 const value=metadata as {gltf?:{extras?:{frames?:unknown;animationDelay?:unknown}};extras?:{frames?:unknown;animationDelay?:unknown}}|null;
 const extras=value?.gltf?.extras||value?.extras;
 const frames=Array.isArray(extras?.frames)?extras.frames.filter((frame):frame is string=>typeof frame==="string"&&!!frame.trim()):[];
 const animationDelay=typeof extras?.animationDelay==="number"?extras.animationDelay:0;
 return frames.length>1&&animationDelay>0?{frames,animationDelay}:undefined;
}

type MaterialAnimation={frames:string[];animationDelay:number};

export function gltfMaterialAnimations(value:unknown):Record<string,MaterialAnimation>{
 const gltf=value as {materials?:Array<{name?:unknown;extras?:unknown}>}|null;
 const animations:Record<string,MaterialAnimation>={};
 for(const material of gltf?.materials||[]){
  if(typeof material.name!=="string")continue;
  const animation=materialAnimation({extras:material.extras});
  if(animation){
   animations[material.name.toLocaleLowerCase()]=animation;
  }
 }
 return animations;
}

export function glbMaterialAnimations(buffer:ArrayBuffer):Record<string,MaterialAnimation>{
 if(buffer.byteLength<20)return {};
 const bytes=new Uint8Array(buffer);
 if(String.fromCharCode(...bytes.slice(0,4))!=="glTF")return {};
 const view=new DataView(buffer);
 const jsonLength=view.getUint32(12,true);
 if(jsonLength<=0||20+jsonLength>buffer.byteLength||String.fromCharCode(...bytes.slice(16,20))!=="JSON")return {};
 try{
  const json=new TextDecoder().decode(bytes.slice(20,20+jsonLength)).replace(/[\u0000 ]+$/g,"");
  return gltfMaterialAnimations(JSON.parse(json));
 }catch{return {}}
}

export function itemTintRgb(value:number){
 const color=value>>>0;
 if(!color)return undefined;
 const linear=(byte:number)=>{
  const channel=byte/255;
  return channel<=.04045?channel/12.92:Math.pow((channel+.055)/1.055,2.4);
 };
 return {r:linear((color>>16)&255),g:linear((color>>8)&255),b:linear(color&255)};
}

export function itemModelCode(item?:InventoryItem){
 const idFile=item?.idFile?.trim().toUpperCase();
 return idFile&&/^IT\d+$/.test(idFile)?idFile:undefined;
}

export function weaponAttachPoint(slot:"primary"|"secondary",itemType?:number,itemName=""){
 // EQSage uses item type 8. The P99 export marks most shields as the unknown
 // sentinel (255), and a small number use unrelated armor types. A semantic
 // fallback keeps shield geometry on the dedicated outside-arm socket.
 if(slot==="secondary"&&(itemType===8||/\b(shield|buckler|aegis)\b/i.test(itemName)))return "shield_point";
 return slot==="primary"?"r_point":"l_point";
}

export function preferredPose<T extends {name:string}>(groups:T[]){
 return groups.find(group=>group.name.toLocaleLowerCase()==="p01")
  ||groups.find(group=>group.name.toLocaleLowerCase()==="pos");
}

export function applyItemBindPose<T extends {reset():unknown}>(groups:T[]){
 // EQ item GLBs store their held orientation in frame zero of a static POS
 // animation. Reset applies that frame once and immediately stops playback,
 // avoiding both the identity bind pose and a continuously replayed pose.
 groups.forEach(group=>group.reset());
}

export function itemEffectAnimationSpeed(group:{from:number;to:number}){
 const durationSeconds=(group.to-group.from)/60;
 return durationSeconds>0?Math.min(.35,durationSeconds/2.5):.35;
}

export function applyStandardPose<T extends {from:number;play(loop?:boolean):unknown;goToFrame(frame:number):unknown;pause():unknown;stop():unknown}>(
 groups:T[],
 preferred:T|undefined,
){
 groups.forEach(group=>group.stop());
 if(!preferred)return;
 // Apply the first frame of the exported P01/POS pose, then hold it. The item
 // material and particle clocks remain independent, so weapon effects keep
 // animating without an idle arm swing carrying held geometry through the body.
 preferred.play(false);
 preferred.goToFrame(preferred.from);
 preferred.pause();
}

export function itemSkeleton<T>(instantiated:T[],container:T[]){
 // Animation groups cloned by instantiateModelsToScene target the cloned
 // skeleton. Replacing it with the container skeleton after applying frame
 // zero discards the held orientation and separates skinned effect planes
 // from their animated transforms.
 return instantiated[0]||container[0];
}

export function CharacterModelViewer({character,profile,items}:{character:string;profile:CharacterModelProfile;items:InventoryItem[]}){
 const canvasRef=useRef<HTMLCanvasElement>(null);
 const[status,setStatus]=useState<"loading"|"ready"|"error">("loading");
 const[error,setError]=useState("");
 const[reload,setReload]=useState(0);
 const[localReady,setLocalReady]=useState<boolean|undefined>(undefined);
 const[source,setSource]=useState<"local"|"online">("online");
 const[appearance,setAppearance]=useState("");
 const modelCode=profile.race+profile.gender;
 const appearanceKey=items.map(item=>`${item.location}:${item.itemId||0}:${item.material||0}:${item.idFile||""}:${item.color||0}`).join("|");

 useEffect(()=>{let active=true;getModelPackStatus().then(value=>{if(active)setLocalReady(value.installed&&value.valid)}).catch(()=>{if(active)setLocalReady(false)});return()=>{active=false}},[]);
 useEffect(()=>{
  if(localReady===undefined)return;
  const canvas=canvasRef.current;
  if(!canvas)return;
  const containWheel=(event:WheelEvent)=>event.preventDefault();
  canvas.addEventListener("wheel",containWheel,{passive:false});
  let disposed=false;
  let engine:import("@babylonjs/core/Engines/engine").Engine|undefined;
  let scene:import("@babylonjs/core/scene").Scene|undefined;
  let observer:ResizeObserver|undefined;
  let intersection:IntersectionObserver|undefined;
  const materialTimers:number[]=[];
  let frame=0,lastFrame=0,visible=true;
  setStatus("loading");setError("");
  (async()=>{
   try{
    const[
     {Engine},{Scene},{Color3,Color4},{ArcRotateCamera},{Vector3},
     {Mesh},
     {HemisphericLight},{DirectionalLight},{SceneLoader},{Texture},
     {Constants},{ParticleSystem},
    ]=await Promise.all([
     import("@babylonjs/core/Engines/engine"),
     import("@babylonjs/core/scene"),
     import("@babylonjs/core/Maths/math.color"),
     import("@babylonjs/core/Cameras/arcRotateCamera"),
     import("@babylonjs/core/Maths/math.vector"),
     import("@babylonjs/core/Meshes/mesh"),
     import("@babylonjs/core/Lights/hemisphericLight"),
     import("@babylonjs/core/Lights/directionalLight"),
     import("@babylonjs/core/Loading/sceneLoader"),
     import("@babylonjs/core/Materials/Textures/texture"),
     import("@babylonjs/core/Engines/constants"),
     import("@babylonjs/core/Particles/particleSystem"),
    ]);
    // Babylon 7 installs Scene.beginDirectAnimation through this side-effect
    // module. EQSage receives it from the full engine bundle; our modular build
    // must register it explicitly before the GLTF loader creates animations.
    await import("@babylonjs/core/Animations/animatable");
    await import("@babylonjs/loaders/glTF");
    if(disposed)return;
    engine=new Engine(canvas,true,MODEL_ENGINE_OPTIONS,true);
    engine.setHardwareScalingLevel(Math.max(1,Math.min(2,window.devicePixelRatio||1)));
    scene=new Scene(engine);
    scene.clearColor=new Color4(0,0,0,0);
    const camera=new ArcRotateCamera("character-camera",DEFAULT_MODEL_ALPHA,Math.PI/2.2,DEFAULT_MODEL_RADIUS,Vector3.Zero(),scene);
    camera.attachControl(canvas,true);
    camera.lowerRadiusLimit=4;
    camera.upperRadiusLimit=20;
    camera.wheelPrecision=40;
    camera.panningSensibility=0;
    camera.inertia=.72;
    const fill=new HemisphericLight("character-fill",new Vector3(0,1,0),scene);
    fill.intensity=1.05;
    const key=new DirectionalLight("character-key",new Vector3(-.5,-1,.7),scene);
    key.intensity=.65;
    const roots=modelRoots(localReady);
    const loadAsset=async(relative:string)=>{
     let failure:unknown;
     for(const root of roots){
      try{return {container:await SceneLoader.LoadAssetContainerAsync(root,relative,scene!),root}}
      catch(reason){failure=reason}
     }
     throw failure||new Error(`No model source was available for ${relative}.`);
    };
    const equipped=visualEquipment(items);
    const bodyModelCode=robeModelCode(modelCode,armorMaterial(equipped.get("chest"),modelCode));
    let result:import("@babylonjs/core/assetContainer").AssetContainer|undefined;
    let lastError:unknown;
    for(const root of roots){
     try{
      result=await SceneLoader.LoadAssetContainerAsync(root,`${bodyModelCode}.glb`,scene);
      setSource(root===LOCAL_MODEL_ROOT?"local":"online");
      break;
     }catch(reason){lastError=reason}
    }
    if(!result)throw lastError||new Error("No model source was available.");
    if(disposed)return;
    const bodyInstance=result.instantiateModelsToScene();
    const bodyAnimations=bodyInstance.animationGroups;
    bodyAnimations.forEach(group=>{group.name=group.name.replace(/^Clone of\s+/,"")});
    let bodyRoot=bodyInstance.rootNodes[0] as import("@babylonjs/core/Meshes/transformNode").TransformNode;
    const bodySkeleton=bodyInstance.skeletons[0];
    const skeletonRoot=bodyRoot.getChildren(undefined,true)[0] as import("@babylonjs/core/Meshes/transformNode").TransformNode|undefined;
    bodyRoot.position.setAll(0);
    bodyRoot.scaling.setAll(1);
    bodyRoot.rotationQuaternion=null;
    bodyRoot.rotation.setAll(0);
    const tintMaterial=(material:import("@babylonjs/core/Materials/material").Material,item:InventoryItem,label:string)=>{
     const tint=itemTintRgb(item.color||0);
     if(!tint||!("albedoColor" in material))return material;
     const clone=material.clone(label) as typeof material&{albedoColor:import("@babylonjs/core/Maths/math.color").Color3};
     clone.albedoColor=new Color3(tint.r,tint.g,tint.b);
     return clone;
    };
    let headLoaded=false;
    let headInstance:{dispose():void}|undefined;
    try{
     const headItem=equipped.get("head");
     const chestItem=equipped.get("chest");
     const headMaterial=headItem?.material||0;
     const headCode=headMaterial>=1&&headMaterial<=23?`${modelCode}he${String(headMaterial).padStart(2,"0")}`:`${modelCode}he00`;
     let loadedHead;
     try{loadedHead=await loadAsset(`${headCode}.glb`)}catch{loadedHead=await loadAsset(`${modelCode}he00.glb`)}
     const head=loadedHead.container.instantiateModelsToScene();
     headInstance=head;
     for(const mesh of head.rootNodes[0]?.getChildMeshes(false)||[]){
      mesh.parent=bodyRoot;
      const material=mesh.material;
      if(material){
       const candidates="subMaterials" in material?(material as unknown as {subMaterials:Array<import("@babylonjs/core/Materials/material").Material|null>}).subMaterials:[material];
       for(let index=0;index<candidates.length;index++){
        const current=candidates[index];
        if(!current||current.name.toLocaleLowerCase().startsWith(`${modelCode}he`))continue;
        const appearanceItem=isRobeHeadMaterial(current.name,bodyModelCode)?chestItem:headItem;
        if(!appearanceItem)continue;
        const tinted=tintMaterial(current,appearanceItem,`${current.name}-${appearanceItem.id}-tint`);
        if("subMaterials" in material)candidates[index]=tinted;else mesh.material=tinted;
       }
      }
     }
     headLoaded=true;
    }catch{/* A base body is still usable when an optional head asset is unavailable. */}
    const mergedBody=Mesh.MergeMeshes(
     bodyRoot.getChildMeshes(false).filter((mesh):mesh is import("@babylonjs/core/Meshes/mesh").Mesh=>mesh instanceof Mesh&&mesh.getTotalVertices()>0),
     true,true,undefined,true,true
    );
    headInstance?.dispose();
    if(!mergedBody||!skeletonRoot||!bodySkeleton)throw new Error("EQSage model merge did not produce a skinned body.");
    skeletonRoot.parent=mergedBody;
    (skeletonRoot as typeof skeletonRoot&{skeleton:typeof bodySkeleton}).skeleton=bodySkeleton;
    bodySkeleton.name="export_model_skeleton";
    bodyRoot.dispose();
    bodyRoot=mergedBody;
    bodyRoot.rotation.y=Math.PI;
    (bodyRoot as typeof bodyRoot&{skeleton:typeof bodySkeleton}).skeleton=bodySkeleton;
    bodyRoot.scaling.z=-1;
    const renderMeshes=[mergedBody];
    const armorSlots:Record<string,string>={ch:"chest",ua:"arms",fa:"wrist",hn:"hands",lg:"legs",ft:"feet",clk:"chest"};
    let armorCount=0;
    for(const mesh of renderMeshes){
     const material=mesh.material;
     if(!material)continue;
     const candidates="subMaterials" in material?(material as unknown as {subMaterials:Array<import("@babylonjs/core/Materials/material").Material|null>}).subMaterials:[material];
     for(let index=0;index<candidates.length;index++){
      const current=candidates[index];
      if(!current||!("albedoTexture" in current))continue;
      const region=/^[a-z]{3}(ch|ua|fa|hn|lg|ft)/i.exec(current.name)?.[1]?.toLocaleLowerCase()||(/^clk/i.test(current.name)?"clk":undefined);
      const item=region?equipped.get(armorSlots[region]):undefined;
      const textureName=item?armorTextureName(current.name,armorMaterial(item,modelCode)):undefined;
      if(!item||!textureName||!replaceableArmorRegion(current.name,modelCode))continue;
      const textureUrl=await firstImageAssetUrl(roots.map(root=>`${root}textures/${textureName}`));
      // EQSage leaves the native material in place when an appearance texture
      // is absent. This also rejects P99 Planner's HTML 200 fallback page.
      if(!textureUrl)continue;
      const clone=current.clone(`${current.name}-${item.id}`) as typeof current&{albedoTexture:unknown;albedoColor?:import("@babylonjs/core/Maths/math.color").Color3};
      clone.albedoTexture=new Texture(textureUrl,scene,false,false,Texture.TRILINEAR_SAMPLINGMODE);
      const tint=itemTintRgb(item.color||0);
      if(tint&&clone.albedoColor)clone.albedoColor=new Color3(tint.r,tint.g,tint.b);
      if("subMaterials" in material)candidates[index]=clone;else mesh.material=clone;
      armorCount++;
     }
    }
    let weaponCount=0;
    for(const slot of ["primary","secondary"]){
     const item=equipped.get(slot);
     const idFile=itemModelCode(item);
     if(!idFile||!/^IT\d+$/i.test(idFile)||!bodySkeleton)continue;
     try{
      const loadedWeapon=await loadAsset(`items/${idFile.toLocaleLowerCase()}.glb`);
      const sourceAnimations=await fetch(`${loadedWeapon.root}items/${idFile.toLocaleLowerCase()}.glb`)
       .then(response=>response.ok?response.arrayBuffer():Promise.reject(new Error(`HTTP ${response.status}`)))
       .then(glbMaterialAnimations)
       .catch(()=>({} as Record<string,MaterialAnimation>));
      const weapon=loadedWeapon.container.instantiateModelsToScene();
      weapon.animationGroups.forEach(group=>{
       group.name=group.name.replace(/^Clone of\s+/,"");
      });
      const weaponSkeleton=itemSkeleton(weapon.skeletons,loadedWeapon.container.skeletons);
      let weaponRoot=weapon.rootNodes[0] as import("@babylonjs/core/Meshes/transformNode").TransformNode|undefined;
      if(!weaponRoot)continue;
      const appearanceOverride=itemAppearance(idFile);
      const particleDefinitions=itemParticleDefinitions(idFile);
      const particleBoneNames=new Set(particleDefinitions.flatMap(effect=>effect.bones.map(name=>name.toLocaleLowerCase())));
      applyItemBindPose(weapon.animationGroups);
      const normalizedGroupName=(name:string)=>name.replace(/^Clone of\s+/i,"").toLocaleLowerCase();
      const positionGroup=weapon.animationGroups.find(group=>normalizedGroupName(group.name)==="pos")||weapon.animationGroups[0];
      const fallbackMotionGroup=(!positionGroup||positionGroup.to<=positionGroup.from)
       ?weapon.animationGroups.find(group=>/^o0\d/i.test(normalizedGroupName(group.name))&&group.to>group.from)
        ||weapon.animationGroups.find(group=>/^p0\d/i.test(normalizedGroupName(group.name))&&group.to>group.from)
       :undefined;
      const itemMotionGroup=fallbackMotionGroup||positionGroup;
      const hasAnimatedItemBones=Boolean(
       itemMotionGroup&&itemMotionGroup.to>itemMotionGroup.from
       &&itemMotionGroup.targetedAnimations.some(targeted=>{
        const targetName=((targeted.target as {name?:string}|undefined)?.name||"").toLocaleLowerCase();
        return !particleBoneNames.has(targetName)&&targeted.animation.getKeys().length>1;
       }),
      );
      // Preserve native bones for every effect-bearing item. Particle direction,
      // animated attachment points, and nested emitters all depend on those
      // transforms; flattening them only happened to work for IT150 because its
      // emitters are direct children of the root.
      const preservesItemHierarchy=Boolean(
       weaponSkeleton&&weaponSkeleton.bones.length>1
       &&(particleDefinitions.length||hasAnimatedItemBones)
       // Nature Walker's Scimitar is the validated exception: all five
       // emitters are direct root children, and flattening keeps its held
       // orientation correct while cloned POS tracks animate the leaves.
       &&idFile.toLocaleUpperCase()!=="IT150",
      );
      const effectGroup=particleDefinitions.length?positionGroup:undefined;
      const particleTracks=new Map<string,{
       node:import("@babylonjs/core/Meshes/transformNode").TransformNode;
       position:import("@babylonjs/core/Maths/math.vector").Vector3;
       animations:import("@babylonjs/core/Animations/animation").Animation[];
      }>();
      if(particleDefinitions.length&&weaponSkeleton){
       weaponSkeleton.prepare();
       for(const particleBone of weaponSkeleton.bones.filter(value=>particleBoneNames.has(value.name.toLocaleLowerCase()))){
        const particleNode=particleBone.getTransformNode();
        if(!particleNode)continue;
        // Particle points are not consistently direct children of the item
        // root. The cleric epic point sits below fourteen chain/ball nodes, so
        // retaining only its local position places the effect at the wrong
        // location once the source hierarchy is merged and disposed.
        particleNode.computeWorldMatrix(true);
        particleTracks.set(particleBone.name.toLocaleLowerCase(),{
         node:particleNode,
         position:particleNode.getAbsolutePosition().clone(),
         animations:(effectGroup?.targetedAnimations||[])
          .filter(targeted=>targeted.target===particleNode&&targeted.animation.targetProperty==="position")
          .map(targeted=>targeted.animation.clone()),
        });
       }
      }
      // Keep the established merged geometry path so held-item orientation and
      // hand placement do not change. Particle tracks captured above survive
      // the source hierarchy being disposed.
      if(!preservesItemHierarchy){
       const mergedWeapon=Mesh.MergeMeshes(
        weaponRoot.getChildMeshes(false).filter((mesh):mesh is import("@babylonjs/core/Meshes/mesh").Mesh=>mesh instanceof Mesh&&mesh.getTotalVertices()>0),
        false,true,undefined,true,true
       );
       if(mergedWeapon){
        weaponRoot.dispose();
        weaponRoot=mergedWeapon;
        (weaponRoot as typeof weaponRoot&{skeleton?:import("@babylonjs/core/Bones/skeleton").Skeleton}).skeleton=weaponSkeleton;
       }
      }
      const point=weaponAttachPoint(slot as "primary"|"secondary",item?.itemType,item?.itemName);
      const bone=bodySkeleton.bones.find(value=>value.name.toLocaleLowerCase()===point);
      const transform=skeletonRoot.getChildTransformNodes().find(value=>value.name.includes(point));
      if(!bone||!transform||!weaponRoot){weaponRoot?.dispose();continue}
      weaponRoot.attachToBone(bone,undefined as never);
      weaponRoot.parent=transform;
      weaponRoot.rotationQuaternion=null;
      weaponRoot.rotation.setAll(0);
      if(appearanceOverride?.rotation)weaponRoot.rotation.set(
       appearanceOverride.rotation.x,
       appearanceOverride.rotation.y,
       appearanceOverride.rotation.z,
      );
      weaponRoot.scaling.setAll(1);
      weaponRoot.name=idFile;
      if(preservesItemHierarchy&&itemMotionGroup&&itemMotionGroup.to>itemMotionGroup.from){
       for(const group of weapon.animationGroups)if(group!==itemMotionGroup)group.stop();
       itemMotionGroup.start(
        true,
        itemEffectAnimationSpeed(itemMotionGroup)*(appearanceOverride?.animationSpeedScale||1),
       );
       itemMotionGroup.goToFrame(itemMotionGroup.from);
      }
      // Item geometry holds its bind pose. Particle position tracks and
      // texture-frame animation continue independently.
      const animatedMaterials=new Set<import("@babylonjs/core/Materials/material").Material>();
      const weaponMeshes=[
       ...(weaponRoot instanceof Mesh?[weaponRoot]:[]),
       ...weaponRoot.getChildMeshes(false).filter((mesh):mesh is import("@babylonjs/core/Meshes/mesh").Mesh=>mesh instanceof Mesh),
      ];
      for(const weaponMesh of weaponMeshes){
       if(!weaponMesh.material)continue;
       animatedMaterials.add(weaponMesh.material);
       if("subMaterials" in weaponMesh.material)for(const material of (weaponMesh.material as unknown as {subMaterials:Array<import("@babylonjs/core/Materials/material").Material|null>}).subMaterials)if(material)animatedMaterials.add(material);
      }
      for(const entry of animatedMaterials){
       if(!entry||!("albedoTexture" in entry))continue;
       // GLTF preserves the EQ shader number but represents additive effects as
       // ordinary alpha blending. Restore only that lost blend operation; masks
       // and all other material behavior remain controlled by the GLTF loader.
       const renderedEntry=entry as typeof entry&{albedoTexture?:import("@babylonjs/core/Materials/Textures/baseTexture").BaseTexture|null};
       if(isAdditiveEqShader(entry.metadata)){
        const additive=renderedEntry as typeof renderedEntry&{
         alphaMode:number;
         useAlphaFromAlbedoTexture?:boolean;
        };
        additive.alphaMode=Constants.ALPHA_ADD;
        additive.useAlphaFromAlbedoTexture=true;
        if(additive.albedoTexture)additive.albedoTexture.hasAlpha=true;
       }
       const animation=materialAnimation(entry.metadata)||sourceAnimations[entry.name.toLocaleLowerCase()];
       if(!animation)continue;
       const baseTexture=renderedEntry.albedoTexture as (import("@babylonjs/core/Materials/Textures/baseTexture").BaseTexture&{
        noMipMap?:boolean;invertY?:boolean;samplingMode?:number;
       })|null|undefined;
       if(!baseTexture)continue;
       const frameNames=animation.frames.map(frame=>frame.toLocaleLowerCase().replace(/\.png$/i,""));
       let animationRoot=loadedWeapon.root;
       if(animationRoot!==REMOTE_MODEL_ROOT){
        const localFrameAvailable=await fetch(`${animationRoot}textures/${frameNames[0]}.png`)
         .then(response=>response.ok)
         .catch(()=>false);
       if(!localFrameAvailable)animationRoot=REMOTE_MODEL_ROOT;
       }
       const frameTextures=frameNames.map(name=>{
        const texture=new Texture(
         `${animationRoot}textures/${name}.png`,
         scene,
         baseTexture.noMipMap,
         baseTexture.invertY,
         baseTexture.samplingMode,
        );
        texture.hasAlpha=baseTexture.hasAlpha;
        return texture;
       });
       let frame=0;
       const advanceFrame=()=>{
        for(let attempt=0;attempt<frameTextures.length;attempt++){
         const next=(frame+1+attempt)%frameTextures.length;
         if(!frameTextures[next].isReady())continue;
         const nextTexture=frameTextures[next];
         frame=next;
         // Assign through Babylon's public material property. Mutating the
         // private GPU handle leaves the material bound to its original frame
         // on some WebView/WebGL combinations.
         renderedEntry.albedoTexture=nextTexture;
         return;
        }
       };
       materialTimers.push(window.setInterval(()=>{
        if(disposed)return;
        advanceFrame();
       },animation.animationDelay*2));
      }
      let particleIndex=0;
      for(const particleEffect of particleDefinitions){
       const sprite=particleEffect.sprite.toLocaleLowerCase();
       const particleTextureUrl=await firstImageAssetUrl(roots.map(root=>`${root}textures/${sprite}.png`))
        ||`${REMOTE_MODEL_ROOT}textures/${sprite}.png`;
       // Original EQ particle sprites commonly use an opaque black background
       // which becomes transparent through additive compositing.
       const particleTexture=new Texture(particleTextureUrl,scene,false,false,Texture.TRILINEAR_SAMPLINGMODE);
       const particleSettings=itemParticleRenderSettings(particleEffect,idFile);
       for(const [emitterIndex,particleName] of particleEffect.bones.entries()){
        const particleTrack=particleTracks.get(particleName.toLocaleLowerCase());
        if(!particleTrack)continue;
        const index=particleIndex++;
        const anchor=new Mesh(`${idFile}-particle-anchor-${index}`,scene);
        anchor.name=`${idFile}-particle-anchor-${particleName}`;
        anchor.parent=preservesItemHierarchy?particleTrack.node:weaponRoot;
        if(preservesItemHierarchy)anchor.position.setAll(0);
        else anchor.position.copyFrom(particleTrack.position);
        if(!preservesItemHierarchy&&effectGroup&&effectGroup.to>effectGroup.from&&particleTrack.animations.length)scene.beginDirectAnimation(
         anchor,
         particleTrack.animations,
         effectGroup.from,
         effectGroup.to,
         true,
         itemEffectAnimationSpeed(effectGroup),
        );
        const particles=new ParticleSystem(`${idFile}-particles-${index}`,particleSettings.capacity,scene);
        particles.particleTexture=particleTexture;
        particles.emitter=anchor;
        particles.updateSpeed=particleSettings.updateSpeed;
        if(particleSettings.direction){
         const direction=new Vector3(...particleSettings.direction);
         particles.createPointEmitter(direction,direction);
         particles.isLocal=true;
         particles.minAngularSpeed=0;
         particles.maxAngularSpeed=0;
        }else{
         particles.createSphereEmitter(particleSettings.radius);
         particles.minAngularSpeed=-2;
         particles.maxAngularSpeed=2;
        }
        particles.emitRate=particleSettings.emitRate;
        particles.minLifeTime=particleSettings.minLifeTime;
        particles.maxLifeTime=particleSettings.maxLifeTime;
        particles.minSize=particleSettings.minSize;
        particles.maxSize=particleSettings.maxSize;
        particles.minScaleX=particleSettings.displayScale;
        particles.maxScaleX=particleSettings.displayScale;
        particles.minScaleY=particleSettings.displayScale;
        particles.maxScaleY=particleSettings.displayScale;
        particles.minEmitPower=particleSettings.minEmitPower;
        particles.maxEmitPower=particleSettings.maxEmitPower;
        particles.gravity=Vector3.Zero();
        const [red,green,blue]=particleSettings.color;
        particles.color1=new Color4(red,green,blue,1);
        particles.color2=new Color4(red,green,blue,1);
        particles.colorDead=new Color4(red,green,blue,0);
        // Preserve the visually approved short Nature Walker leaf lifecycle.
        if(idFile.toLocaleUpperCase()==="IT150"){
         const leafSize=particleSettings.sizeMultiplier;
         particles.addColorGradient(0,new Color4(1,1,1,.75));
         particles.addColorGradient(.08,new Color4(1,1,1,.75));
         particles.addColorGradient(.38,new Color4(1,1,1,.3));
         particles.addColorGradient(1,new Color4(1,1,1,0));
         particles.addSizeGradient(0,leafSize);
         particles.addSizeGradient(.4,leafSize*.78);
         particles.addSizeGradient(1,leafSize*.15);
        }
        particles.blendMode=ParticleSystem.BLENDMODE_ADD;
        particles.start(itemParticleStartDelay(idFile,emitterIndex,particleEffect.bones.length,particleSettings.emitRate));
       }
      }
      weaponCount++;
     }catch{/* Unknown item models remain represented by their paper-doll icon. */}
    }
    setAppearance(`${headLoaded?"Head":"Base"}${armorCount?` - ${armorCount} armor`:""}${weaponCount?` - ${weaponCount} weapon${weaponCount===1?"":"s"}`:""}`);
    let minimum=new Vector3(Number.POSITIVE_INFINITY,Number.POSITIVE_INFINITY,Number.POSITIVE_INFINITY);
    let maximum=new Vector3(Number.NEGATIVE_INFINITY,Number.NEGATIVE_INFINITY,Number.NEGATIVE_INFINITY);
    for(const mesh of renderMeshes){
     mesh.computeWorldMatrix(true);
     const bounds=mesh.getBoundingInfo().boundingBox;
     minimum=Vector3.Minimize(minimum,bounds.minimumWorld);
     maximum=Vector3.Maximize(maximum,bounds.maximumWorld);
    }
    const center=minimum.add(maximum).scale(.5);
    const size=maximum.subtract(minimum);
    camera.setTarget(center.add(new Vector3(0,size.y*MODEL_VERTICAL_OFFSET,0)));
    camera.alpha=DEFAULT_MODEL_ALPHA;
    camera.radius=DEFAULT_MODEL_RADIUS;
    const idle=preferredPose(bodyAnimations);
    applyStandardPose(bodyAnimations,idle);
    // Commit an initial frame immediately. The throttled/visibility-aware loop
    // handles subsequent animation, but should never leave a ready canvas blank.
    scene.render();
    observer=new ResizeObserver(()=>engine?.resize());
    observer.observe(canvas as unknown as Element);
    intersection=new IntersectionObserver(entries=>{visible=entries[0]?.isIntersecting??true});
    intersection.observe(canvas as unknown as Element);
    const render=(time:number)=>{
     if(disposed)return;
     if(visible&&time-lastFrame>=32){scene?.render();lastFrame=time}
     frame=requestAnimationFrame(render);
    };
    frame=requestAnimationFrame(render);
    setStatus("ready");
   }catch(reason){
    if(!disposed){setError(String(reason).replace(/^Error:\s*/,""));setStatus("error")}
   }
  })();
  return ()=>{
   disposed=true;
   cancelAnimationFrame(frame);
   observer?.disconnect();
   intersection?.disconnect();
   for(const timer of materialTimers)window.clearInterval(timer);
   canvas.removeEventListener("wheel",containWheel);
   scene?.dispose();
   engine?.dispose();
  };
 },[modelCode,appearanceKey,reload,localReady]);

 return <div className="character-model-viewer">
  <canvas ref={canvasRef} aria-label={`Interactive 3D model for ${character}`}/>
  {status==="loading"&&<div className="character-model-status"><i/><strong>Loading EQ model...</strong></div>}
  {status==="error"&&<div className="character-model-status error"><strong>Model unavailable</strong><span>{error}</span><button onClick={()=>setReload(value=>value+1)}>Retry</button></div>}
  {status==="ready"&&<div className="character-model-hint">{source==="local"?"Local pack":"Online fallback"} - {appearance} - drag to rotate - wheel to zoom</div>}
 </div>;
}
