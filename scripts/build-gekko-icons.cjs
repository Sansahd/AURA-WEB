'use strict';
/*
 * Canonical source: user-uploaded src/assets/gekko-user-logo.webp.b64
 * Generate Windows ICO (seven sizes) plus runtime PNG from user's artwork.
 * This script runs in CI before native packaging. Never package Electron's
 * fallback EXE/logo if an icon cannot be rendered.
 */
const fs=require('node:fs');
const path=require('node:path');
const sharp=require('sharp');
const root=path.resolve(__dirname,'..');
const userArtwork=path.join(root,'src','assets','gekko-user-logo.webp.b64');
const noirArtwork=path.join(root,'src','assets','gekko-origami-noir.svg');
const pngFile=path.join(root,'src','assets','gekko-icon.png');
const icoFile=path.join(root,'build','gekko.ico');
const sizes=[16,24,32,48,64,128,256];

async function build(){
 const encoded=fs.readFileSync(userArtwork,'utf8').trim();
 if(!/^[A-Za-z0-9+/=]+$/.test(encoded))throw Error('Invalid uploaded logo encoding');
 const uploadedArtwork=Buffer.from(encoded,'base64');
 const artwork=fs.existsSync(noirArtwork) ? fs.readFileSync(noirArtwork) : uploadedArtwork;
 if(uploadedArtwork.length<4000 || uploadedArtwork.toString('ascii',0,4)!=='RIFF' ||
    uploadedArtwork.toString('ascii',8,12)!=='WEBP')
   throw Error('GEKKO user artwork not a valid WebP');
 const meta=await sharp(artwork).metadata();
 if(meta.width<80 || meta.height<80 || (!meta.hasAlpha && !fs.existsSync(noirArtwork)))
   throw Error('GEKKO user artwork must be a transparent square');
 const pngs=await Promise.all(sizes.map(size=>
   sharp(artwork).resize(size,size,{fit:'contain',background:'#00000000'})
     .png({compressionLevel:9}).toBuffer()));
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
