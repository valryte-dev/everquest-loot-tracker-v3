import {toBlob} from "html-to-image";

export type FightImageCopyResult="clipboard"|"download";

function safeFileName(value:string){return value.replace(/[<>:"/\\|?*\u0000-\u001f]/g,"-").replace(/\s+/g," ").trim()||"fight-report"}

function downloadBlob(blob:Blob,fileName:string){
 const url=URL.createObjectURL(blob),anchor=document.createElement("a");
 anchor.href=url;anchor.download=safeFileName(fileName);document.body.appendChild(anchor);anchor.click();anchor.remove();
 window.setTimeout(()=>URL.revokeObjectURL(url),1000);
}

export async function copyFightPanelAsPng(element:HTMLElement,fileName:string):Promise<FightImageCopyResult>{
 await document.fonts?.ready;
 const width=Math.ceil(Math.max(element.clientWidth,element.scrollWidth));
 const height=Math.ceil(Math.max(element.clientHeight,element.scrollHeight));
 if(!width||!height)throw new Error("The fight panel is not ready to capture.");
 const maxDimension=Math.max(width,height),pixelRatio=Math.min(2,Math.max(1,12000/maxDimension));
 const backgroundColor=getComputedStyle(document.documentElement).getPropertyValue("--panel").trim()||"#141817";
 const blob=await toBlob(element,{cacheBust:true,backgroundColor,width,height,pixelRatio});
 if(!blob)throw new Error("The fight image renderer returned an empty image.");
 if(navigator.clipboard?.write&&typeof ClipboardItem!=="undefined"){
  try{
   await navigator.clipboard.write([new ClipboardItem({"image/png":blob})]);
   return "clipboard";
  }catch(error){
   console.warn("Image clipboard unavailable; downloading the rendered fight report instead",error);
  }
 }
 downloadBlob(blob,fileName);
 return "download";
}