import test from 'node:test';
import assert from 'node:assert/strict';
import { destination, frame, CAT_HEIGHT, CAT_WIDTH, DURATION, walkingFrame } from '../src/animation.ts';
test('目标约束保持角色在屏幕内', () => {
  const point = destination({x:-100,y:4000},1440,900);
  assert.ok(point.x-CAT_WIDTH/2>=0);assert.ok(point.y<=900);assert.ok(point.y-CAT_HEIGHT>=0);
});
test('两种方向的动画都准确结束且变形恢复', () => {
  for (const x of [100,900]) {
    const pose = frame({x:500,y:600},{x,y:300},DURATION);
    assert.equal(pose.x,x);assert.equal(pose.y,300);assert.equal(pose.sx,1);assert.equal(pose.sy,1);assert.equal(pose.rotation,0);assert.equal(pose.done,true);assert.equal(pose.flipped,x<500);
  }
});
test('运动到撞击和反弹阶段保持连续', () => {
  const from={x:100,y:700},to={x:800,y:300};
  for (const boundary of [.14,.44,.49,.60,.84,.94]) {
    const a=frame(from,to,DURATION*boundary-.001),b=frame(from,to,DURATION*boundary+.001);
    assert.ok(Math.hypot(a.x-b.x,a.y-b.y)<.1);
  }
  assert.ok(frame(from,to,DURATION*.46).sy>1);
  assert.ok(frame(from,to,DURATION*.72).y<to.y);
});

test('独立步行动画无撞击变形并准确停下',()=>{
 const from={x:100,y:600},to={x:800,y:300};
 for(let elapsed=0;elapsed<=1100;elapsed+=10){const p=walkingFrame(from,to,elapsed);assert.equal(p.impact,-1);assert.equal(p.sx,1);assert.equal(p.sy,1);assert.ok(p.x>=100&&p.x<=800);}
 const rest=walkingFrame(from,to,1100);assert.equal(rest.x,to.x);assert.equal(rest.y,to.y);assert.equal(rest.done,true);
});

test('撞击停顿保持压扁姿势，随后产生明显弹退',()=>{
 const from={x:100,y:600},to={x:800,y:300};
 const a=frame(from,to,DURATION*.45),b=frame(from,to,DURATION*.48);
 assert.equal(a.x,b.x);assert.equal(a.y,b.y);assert.equal(a.sx,.5);assert.equal(a.sy,1.38);
 const recoil=frame(from,to,DURATION*.72);assert.ok(to.x-recoil.x>70);assert.ok(to.y-recoil.y>60);assert.ok(Math.abs(recoil.rotation)>.35);
});
