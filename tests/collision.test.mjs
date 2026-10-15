import test from 'node:test';
import assert from 'node:assert/strict';
import {contact,displacement,NOSE} from '../src/collision.ts';
test('任意点击位置都以鼠标坐标作为鼻尖接触点',()=>{
 const rect={x:400,y:300,width:120,height:40};
 for(const target of [{rect},null,undefined]){
  for(const from of [{x:100,y:600},{x:900,y:600}]){
   const hit=contact(from,{x:450,y:320,target});
   assert.deepEqual(hit.point,{x:450,y:320});
   assert.equal(hit.anchor.x+hit.direction*NOSE.x,450);
   assert.equal(hit.anchor.y+NOSE.y,320);
  }
 }
});
test('画面边缘固定，目标像素受撞后恢复',()=>{
 const rect={x:100,y:100,width:100,height:40},patch={x:36,y:36,width:228,height:168};
 for(const p of [{x:36,y:100},{x:264,y:100},{x:130,y:36},{x:130,y:204}])assert.deepEqual(displacement(p,rect,patch,.12,1),p);
 const p={x:150,y:120};assert.notDeepEqual(displacement(p,rect,patch,.12,1),p);
 for(const t of [0,1])assert.deepEqual(displacement(p,rect,patch,t,1),p);
});
