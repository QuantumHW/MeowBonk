export type Point = { x: number; y: number };
export const CAT_WIDTH = 196;
export const CAT_HEIGHT = CAT_WIDTH * 1105 / 1423;
export const DURATION = 2100;
export const HIT = .44;
export const IMPACT_END = .94;
export const clamp = (value: number, min: number, max: number) => Math.max(min, Math.min(max, value));
export function destination(point: Point, width: number, height: number): Point {
  return { x: clamp(point.x, CAT_WIDTH / 2 + 8, Math.max(CAT_WIDTH / 2 + 8, width - CAT_WIDTH / 2 - 8)), y: clamp(point.y, CAT_HEIGHT + 8, Math.max(CAT_HEIGHT + 8, height - 12)) };
}
export function frame(from: Point, to: Point, elapsed: number) {
  const t = clamp(elapsed / DURATION, 0, 1);
  const direction = to.x >= from.x ? 1 : -1;
  let x = from.x, y = from.y, rotation = 0, sx = 1, sy = 1;
  if (t < .14) {
    const p=t/.14, charge=Math.sin(p*Math.PI/2);
    x -= direction*24*charge; y += 9*charge;
    rotation=-direction*.13*charge; sx=1-.14*charge; sy=1-.17*charge;
  } else if (t < HIT) {
    const p=(t-.14)/(HIT-.14), eased=p*p*p;
    x=from.x-direction*24+(to.x-from.x+direction*24)*eased;
    y=from.y+9+(to.y-from.y-9)*eased-Math.abs(Math.sin(p*Math.PI*7))*14;
    rotation=direction*.10*Math.sin(p*Math.PI*14);
    sx=.86+.52*p; sy=.83+.03*p;
  } else if (t < .49) {
    // Hold the crushed pose for 105 ms: the impact must read before the rebound.
    x=to.x; y=to.y; sx=.50; sy=1.38;
  } else if (t < .60) {
    const p=(t-.49)/.11, ease=p*p*(3-2*p);
    x=to.x; y=to.y; sx=.50+.50*ease+.18*Math.sin(p*Math.PI); sy=1.38-.38*ease;
  } else if (t < .84) {
    const p=(t-.60)/.24, bounce=Math.sin(p*Math.PI);
    x=to.x-direction*82*bounce; y=to.y-70*bounce;
    rotation=-direction*.40*bounce; sx=1+.18*Math.sin(p*Math.PI*2)*bounce; sy=1-.12*bounce;
  } else if (t < IMPACT_END) {
    const p=(t-.84)/(IMPACT_END-.84), wobble=Math.sin(p*Math.PI*4)*(1-p);
    x=to.x; y=to.y; rotation=direction*.07*wobble; sx=1+.08*wobble; sy=1-.06*wobble;
  } else {x=to.x;y=to.y;}
  return {x,y,rotation,sx,sy,flipped:direction<0,impact:t>=HIT&&t<IMPACT_END?(t-HIT)/(IMPACT_END-HIT):-1,done:elapsed>=DURATION};
}

export function walkingFrame(from: Point, to: Point, elapsed: number) {
  const duration = clamp(Math.hypot(to.x-from.x,to.y-from.y)*1.1,380,950);
  const t = clamp(elapsed/duration,0,1), eased=t*t*(3-2*t), gait=Math.sin(t*Math.PI*8)*Math.sin(t*Math.PI);
  return {x:from.x+(to.x-from.x)*eased,y:from.y+(to.y-from.y)*eased-Math.abs(gait)*6,rotation:gait*.025,sx:1,sy:1,flipped:to.x<from.x,impact:-1,done:elapsed>=duration};
}
