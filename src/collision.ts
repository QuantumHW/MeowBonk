import { CAT_WIDTH, CAT_HEIGHT, clamp, type Point } from './animation.ts';
export type Rect = Point & { width: number; height: number };
export type Collision = Point & { target?: { rect: Rect; surface?: { rect: Rect; dataUrl: string } | null } | null };
// Nose anchor in the standing sprite; the foot anchor remains the render origin.
export const NOSE = { x: CAT_WIDTH * .29, y: -CAT_HEIGHT * .67 };
export function contact(from: Point, packet: Collision) {
  const flipped = packet.x < from.x;
  const direction = flipped ? -1 : 1;
  const point = { x: packet.x, y: packet.y };
  return { point, anchor: {x:point.x-direction*NOSE.x,y:point.y-NOSE.y}, direction };
}
export function displacement(point: Point, rect: Rect, patch: Rect, impact: number, direction: number): Point {
  if (impact <= 0 || impact >= 1) return {...point};
  const dx = Math.max(rect.x-point.x,0,point.x-rect.x-rect.width);
  const dy = Math.max(rect.y-point.y,0,point.y-rect.y-rect.height);
  const edge = Math.min(point.x-patch.x,patch.x+patch.width-point.x,point.y-patch.y,patch.y+patch.height-point.y);
  const weight = clamp(1-Math.hypot(dx,dy)/58,0,1) * clamp(edge/12,0,1);
  // A sharp kick, a readable hold, then a damped overshoot; keep the padded boundary fixed.
  const attack=clamp(impact/.035,0,1), release=clamp((impact-.10)/.90,0,1);
  const spring=impact<.10 ? attack*attack*(3-2*attack) : Math.cos(release*Math.PI*3.5)*Math.exp(-release*4.5)*(1-release);
  const cx=point.x-rect.x-rect.width/2,cy=point.y-rect.y-rect.height/2;
  const angle=spring*Math.min(.14,18/Math.max(rect.width,rect.height));
  const squeeze=spring*Math.min(.32,36/rect.width), stretch=spring*Math.min(.28,24/rect.height);
  const localX=cx*(1-squeeze), localY=cy*(1+stretch);
  return {x:point.x+weight*(direction*32*spring+localX*Math.cos(angle)-localY*Math.sin(angle)-cx),y:point.y+weight*(localX*Math.sin(angle)+localY*Math.cos(angle)-cy-7*spring)};

}
// The padded patch supplies background pixels, so displaced controls do not leave a duplicate behind.
export function drawSurface(ctx: CanvasRenderingContext2D, image: HTMLImageElement, rect: Rect, patch: Rect, impact: number, direction: number) {
  if (impact < 0 || impact >= 1) return;
  const cols = Math.ceil(patch.width/24), rows = Math.ceil(patch.height/24);
  const triangle = (a:Point,b:Point,c:Point) => {
    const p=displacement(a,rect,patch,impact,direction),q=displacement(b,rect,patch,impact,direction),r=displacement(c,rect,patch,impact,direction);
    const det=(b.x-a.x)*(c.y-a.y)-(c.x-a.x)*(b.y-a.y);
    const aa=((q.x-p.x)*(c.y-a.y)-(r.x-p.x)*(b.y-a.y))/det;
    const bb=((q.y-p.y)*(c.y-a.y)-(r.y-p.y)*(b.y-a.y))/det;
    const cc=((r.x-p.x)*(b.x-a.x)-(q.x-p.x)*(c.x-a.x))/det;
    const dd=((r.y-p.y)*(b.x-a.x)-(q.y-p.y)*(c.x-a.x))/det;
    ctx.save();
    // A slight overlap prevents antialiasing hairlines between adjacent triangles.
    const center={x:(p.x+q.x+r.x)/3,y:(p.y+q.y+r.y)/3};
    const expand=(v:Point)=>({x:v.x+(v.x-center.x)*.025,y:v.y+(v.y-center.y)*.025});
    const pp=expand(p),qq=expand(q),rr=expand(r);
    ctx.beginPath();ctx.moveTo(pp.x,pp.y);ctx.lineTo(qq.x,qq.y);ctx.lineTo(rr.x,rr.y);ctx.closePath();ctx.clip();
    ctx.transform(aa,bb,cc,dd,p.x-aa*a.x-cc*a.y,p.y-bb*a.x-dd*a.y);
    ctx.drawImage(image,patch.x,patch.y,patch.width,patch.height);ctx.restore();
  };
  for(let y=0;y<rows;y++) for(let x=0;x<cols;x++) {
    const a={x:patch.x+x*patch.width/cols,y:patch.y+y*patch.height/rows};
    const b={x:a.x+patch.width/cols,y:a.y}, c={x:a.x,y:a.y+patch.height/rows},d={x:b.x,y:c.y};
    triangle(a,b,c);triangle(b,d,c);
  }
}
