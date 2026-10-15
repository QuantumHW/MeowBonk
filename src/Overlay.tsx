import { useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { drawImpact, drawRush } from './impact-effects.ts';
import { contact, drawSurface, type Collision } from './collision.ts';
import { CAT_WIDTH, CAT_HEIGHT, DURATION, HIT, IMPACT_END, clamp, destination, frame, type Point } from './animation.ts';

export function Overlay() {
  const canvas = useRef<HTMLCanvasElement>(null);
  const hitTarget = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const element = canvas.current!;
    const context = element.getContext('2d')!;
    const image = new Image();
    let position: Point = destination({ x: 180, y: innerHeight - 70 }, innerWidth, innerHeight);
    let flipped = false, animation = 0, disposed = false, ready = false, generation = 0;
    let active: Collision | null = null, surface: HTMLImageElement | null = null, direction = 1, rush = -1;
    const cleanups: (() => void)[] = [];
    const bounds = () => {
      Object.assign(hitTarget.current!.style, { display: 'block', left: `${position.x - CAT_WIDTH / 2}px`, top: `${position.y - CAT_HEIGHT}px`, width: `${CAT_WIDTH}px`, height: `${CAT_HEIGHT}px` });
      return invoke('set_pet_bounds', { bounds: { x: position.x - CAT_WIDTH / 2, y: position.y - CAT_HEIGHT, width: CAT_WIDTH, height: CAT_HEIGHT, flipped } });
    };
    const resize = () => {
      const dpr = devicePixelRatio;
      element.width = Math.round(innerWidth * dpr); element.height = Math.round(innerHeight * dpr);
      context.setTransform(dpr, 0, 0, dpr, 0, 0);
    };
    const paint = (rotation = 0, sx = 1, sy = 1, impact = -1, target = position) => {
      context.clearRect(0, 0, innerWidth, innerHeight);
      if (impact >= 0 && active?.target && surface) drawSurface(context, surface, active.target.rect, active.target.surface!.rect, impact, direction);
      drawRush(context,position,direction,rush);
      context.save(); context.translate(position.x, position.y); context.rotate(rotation); context.scale((flipped ? -1 : 1) * sx, sy);
      context.drawImage(image, -CAT_WIDTH / 2, -CAT_HEIGHT, CAT_WIDTH, CAT_HEIGHT); context.restore();
      drawImpact(context,target,impact,direction);
    };
    const settle = () => { generation++; rush = -1; active = null; surface = null; cancelAnimationFrame(animation); animation = 0; position = destination(position, innerWidth, innerHeight); paint(); void bounds().catch(console.error); };
    const bonk = async (point: Collision) => {
      if (!ready || !Number.isFinite(point.x) || !Number.isFinite(point.y)) return;
      if (!point.target?.surface) return;
      settle();
      const token = ++generation, patch = new Image();
      patch.src = point.target.surface.dataUrl;
      try { await patch.decode(); }
      catch { if (!disposed && token === generation) void invoke('report_visual_failure').catch(console.error); return; }
      if (disposed || token !== generation) return;
      // Start both effects with a decoded screenshot; late images never join halfway through.
      cancelAnimationFrame(animation); void invoke('set_pet_bounds', { bounds: null }).catch(console.error);
      hitTarget.current!.style.display = 'none';
      active = point; surface = patch;
      const from = { ...position };
      const collision = contact(from, point), target = collision.anchor, started = performance.now();
      direction = collision.direction;
      const tick = (now: number) => {
        if (disposed) return;
        const elapsed = now-started, pose = frame(from, target, elapsed); position = { x: pose.x, y: pose.y }; flipped = direction < 0;
        rush = elapsed/DURATION>=.14 && elapsed/DURATION<HIT ? (elapsed/DURATION-.14)/(HIT-.14) : -1;
        if (elapsed >= DURATION*IMPACT_END) { const rest=destination(target,innerWidth,innerHeight),p=clamp((elapsed/DURATION-IMPACT_END)/(1-IMPACT_END),0,1),ease=p*p*(3-2*p);position.x+=(rest.x-target.x)*ease;position.y+=(rest.y-target.y)*ease; }
        // Squash around the nose rather than pulling the contact point away from the click.
        const noseX = direction * CAT_WIDTH * .29, noseY = -CAT_HEIGHT * .67;
        const scaledX=noseX*pose.sx,scaledY=noseY*pose.sy;
        position.x += noseX-(scaledX*Math.cos(pose.rotation)-scaledY*Math.sin(pose.rotation));
        position.y += noseY-(scaledX*Math.sin(pose.rotation)+scaledY*Math.cos(pose.rotation));
        paint(pose.rotation, pose.sx, pose.sy, pose.impact, collision.point);
        if (pose.done) settle(); else animation = requestAnimationFrame(tick);
      };
      animation = requestAnimationFrame(tick);
    };
    async function attach() {
      for (const [event, callback] of [
        ['bonk', (e: { payload: Collision }) => { void bonk(e.payload); }], ['pause', () => { if (ready) settle(); }],
      ] as const) {
        const unlisten = await listen(event, callback as Parameters<typeof listen>[1]);
        if (disposed) unlisten(); else cleanups.push(unlisten);
      }
      if (disposed) return;
      image.onload = () => { if (disposed) return; ready = true; resize(); paint(); void bounds().then(() => invoke('pet_ready')).catch(console.error); };
      image.src = '/cat-standing.png';
    }
    void attach().catch(console.error);
    const onResize = () => { if (ready) { resize(); settle(); } };
    window.addEventListener('resize', onResize);
    return () => { disposed = true; cancelAnimationFrame(animation); cleanups.forEach(fn => fn()); window.removeEventListener('resize', onResize); image.onload = null; };
  }, []);
  return <><canvas ref={canvas} className="overlay" aria-hidden="true"/><div ref={hitTarget} role="img" aria-label="MeowBonk 小猫，右击打开菜单" style={{position:'fixed', display:'none'}} onContextMenu={event => { event.preventDefault(); void invoke('pet_menu').catch(console.error); }}/></>;
}
