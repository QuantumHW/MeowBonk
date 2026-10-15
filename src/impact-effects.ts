import { clamp, type Point } from './animation.ts';
export function drawImpact(ctx:CanvasRenderingContext2D, contact:Point, impact:number, direction:number) {
  if (impact<0 || impact>=1) return;
  ctx.save();ctx.translate(contact.x,contact.y);
  const flash=clamp(1-impact/.30,0,1), burst=clamp(impact/.06,0,1);
  if (flash>0) {
    ctx.save();ctx.globalAlpha=flash;ctx.rotate(-direction*.13);
    ctx.beginPath();
    for(let i=0;i<24;i++) {const a=i*Math.PI/12,r=(i%2?28:58+(i%3)*8)*(.65+.35*burst);if(i===0)ctx.moveTo(Math.cos(a)*r,Math.sin(a)*r);else ctx.lineTo(Math.cos(a)*r,Math.sin(a)*r);}
    ctx.closePath();ctx.fillStyle='#ffe45c';ctx.strokeStyle='#30251d';ctx.lineWidth=4;ctx.fill();ctx.stroke();
    ctx.beginPath();ctx.arc(0,0,13+burst*12,0,Math.PI*2);ctx.fillStyle='#fff9da';ctx.fill();ctx.restore();
  }
  // Broken shock rays emphasize contact without pretending to shake the whole desktop.
  ctx.globalAlpha=clamp(1-impact/.65,0,1);ctx.lineCap='round';
  for(let i=0;i<10;i++) {const a=i*Math.PI/5+.12,r=48+impact*130;ctx.beginPath();ctx.moveTo(Math.cos(a)*r,Math.sin(a)*r);ctx.lineTo(Math.cos(a)*(r+18),Math.sin(a)*(r+18));ctx.strokeStyle='#fff4c8';ctx.lineWidth=7;ctx.stroke();ctx.strokeStyle='#e99c29';ctx.lineWidth=3;ctx.stroke();}
  const textAlpha=clamp((1-impact)/.25,0,1), rise=Math.sin(clamp(impact/.24,0,1)*Math.PI/2);
  ctx.globalAlpha=textAlpha;ctx.translate(direction*26,-58-rise*30);ctx.rotate(-direction*.13);
  const scale=.7+.4*rise;ctx.scale(scale,scale);ctx.font='900 italic 39px system-ui, sans-serif';ctx.textAlign='center';ctx.textBaseline='middle';ctx.lineJoin='round';ctx.strokeStyle='#34221b';ctx.lineWidth=7;ctx.strokeText('BONK!',0,0);ctx.fillStyle='#fff0a1';ctx.fillText('BONK!',0,0);
  ctx.restore();
}
export function drawRush(ctx:CanvasRenderingContext2D, position:Point, direction:number, progress:number) {
  if(progress<=0 || progress>=1) return;
  ctx.save();ctx.globalAlpha=Math.sin(progress*Math.PI)*.9;ctx.lineCap='round';
  for(let i=0;i<4;i++) {const x=position.x-direction*(95+i%2*15), y=position.y-30-i*24;ctx.beginPath();ctx.moveTo(x,y);ctx.lineTo(x-direction*(35+progress*90+i*8),y+4);ctx.strokeStyle='#fff2bf';ctx.lineWidth=6-i*.6;ctx.stroke();}
  ctx.restore();
}
