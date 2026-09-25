import {test} from 'node:test';
import assert from 'node:assert/strict';
import {progress,mergeDays} from '../lib/consistency';
test('completion is relative to each routine and missing days remain unknown',()=>{
 const a=progress([{date:'2026-09-25',completed:1,target:2,updatedAt:'2026-09-25T12:00:00Z'}],'2026-09-25');
 const b=progress([{date:'2026-09-25',completed:10,target:20,updatedAt:'2026-09-25T12:00:00Z'}],'2026-09-25');
 assert.equal(a.latestPercent,50);assert.equal(b.latestPercent,50);assert.equal(a.heatmap[0].ratio,null);assert.equal(a.delta,null);
});
test('delta compares tracked days in completed weeks, without penalizing today',()=>{
 const p=progress([{date:'2026-09-15',completed:1,target:4,updatedAt:'2026-09-15T12:00:00Z'},{date:'2026-09-23',completed:3,target:4,updatedAt:'2026-09-23T12:00:00Z'},{date:'2026-09-25',completed:0,target:4,updatedAt:'2026-09-25T12:00:00Z'}],'2026-09-25');
 assert.equal(p.delta,50);assert.equal(p.latestPercent,0);
});
test('routine upload retries cannot overwrite a newer snapshot and storage stays bounded',()=>{
 const newer={date:'2026-09-25',completed:3,target:4,updatedAt:'2026-09-25T14:00:00Z'};
 const result=mergeDays([newer],[{...newer,completed:1,updatedAt:'2026-09-25T12:00:00Z'},{...newer,date:'2020-01-01'}],'2026-09-25');assert.deepEqual(result,[newer]);
});
