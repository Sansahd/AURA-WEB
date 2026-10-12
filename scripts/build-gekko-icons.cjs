'use strict';
/*
 * Canonical source: src/assets/quantic-glide-mark.svg
 * Generate Windows ICO (seven sizes) plus runtime PNG with the same vector.
 * This script runs in CI before native packaging. Never package Electron's
 * fallback EXE/logo if an icon cannot be rendered.
 */
const fs=require('node:fs');
const path=require('node:path');
const sharp=require('sharp');
const root=path.resolve(__dirname,'..');
const svgFile=path.join(root,'src','assets','quantic-glide-mark.svg');
const pngFile=path.join(root,'src','assets','gekko-icon.png');
const icoFile=path.join(root,'build','gekko.ico');
const sizes=[16,24,32,48,64,128,256];

async function build(){
 const svg=fs.readFileSync(svgFile);
 if(!svg.toString('utf8').includes('id="gecko"') ||
    !svg.toString('utf8').includes('id="earth"'))throw Error('GEKKO art missing gecko or globe');
 const pngs=await Promise.all(sizes.map(size=>
   sharp(svg,{density:384}).resize(size,size).png({compressionLevel:9}).toBuffer()));
 const large=pngs[pngs.length-1];
 const dir=Buffer.alloc(6+sizes.length*16);
 dir.writeUInt16LE(0,0);dir.writeUInt16LE(1,2);dir.writeUInt16LE(sizes.length,4);
 let offset=dir.length;
 for(let i=0;i<sizes.length;i++){
   const idx=6+i*16,size=sizes[i],png=pngs[i];
   dir.writeUInt8(size===256?0:size,idx);
   dir.writeUInt8(size===256?0:size,idx+1);
   dir.writeUInt8(0,idx+2);
   dir.writeUInt8(0,idx+3);
   dir.writeUInt16LE(1,idx+4);
   dir.writeUInt16LE(32,idx+6);
   dir.writeUInt32LE(png.length,idx+8);
   dir.writeUInt32LE(offset,idx+12);
   offset+=png.length;
 }
 fs.mkdirSync(path.dirname(icoFile),{recursive:true});
 fs.writeFileSync(pngFile,large);
 fs.writeFileSync(icoFile,Buffer.concat([dir,...pngs]));
 if(fs.statSync(icoFile).size<10000)throw Error('GEKKO Windows icon too small');
 console.log('[branding] Generated GEKKO icon: '+sizes.join(', ')+'px, '+fs.statSync(icoFile).size+' bytes');
}
build().catch(error=>{console.error(error);process.exitCode=1});
