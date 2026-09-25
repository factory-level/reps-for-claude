import {RoutineEditor} from './RoutineEditor';
import {useEffect,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import type {Snapshot} from './snapshot';
type Status={agents:{codex:boolean;claude:boolean};snoozeUntil:number;workMinutes:number;lockMode:boolean;sync:string};
type Record={id:string;date:string;exercise:string;reps:number;seconds:number;weight:number;weightUnit:string|null};
export function DailyPanel({snapshot}:{snapshot:Snapshot}){
 const [status,setStatus]=useState<Status|null>(null),[error,setError]=useState(''),[minutes,setMinutes]=useState(25),[lock,setLock]=useState(false),[loaded,setLoaded]=useState(false);
 const [records,setRecords]=useState<Record[]>([]),[busy,setBusy]=useState(false);
 useEffect(()=>{let active=true;const poll=()=>void invoke<Status>('daily_status').then(s=>{if(active)setStatus(s);}).catch(e=>{if(active)setError(String(e));});poll();const timer=setInterval(poll,3000);return()=>{active=false;clearInterval(timer);};},[]);
 useEffect(()=>{if(status&&!loaded){setMinutes(status.workMinutes);setLock(status.lockMode);setLoaded(true);}},[status,loaded]);
 useEffect(()=>{let active=true;void invoke<{records:Record[]}>('recent_history').then(v=>{if(active)setRecords(v.records);}).catch(e=>{if(active)setError(String(e));});return()=>{active=false;};},[snapshot.phase]);
 async function act(command:string,args?:{[key:string]:unknown}){setBusy(true);setError('');try{await invoke(command,args);setStatus(await invoke<Status>('daily_status'));}catch(e){setError(String(e));}finally{setBusy(false);}}
 const resting=snapshot.phase==='CODING'||snapshot.phase==='EXERCISE_REQUIRED';
 return <section className="daily-panel"><div><strong>RFP · reps for prompts</strong><p>{status?`Codex ${status.agents.codex?'open':'closed'} · Claude ${status.agents.claude?'open':'closed'}`:'Checking your agents…'}</p>
 {status&&!(status.agents.codex||status.agents.claude)?<p>Timer paused. Reps is waiting quietly for Codex or Claude.</p>:null}
 {status&&status.snoozeUntil>Date.now()/1000?<p>Snoozed until {new Date(status.snoozeUntil*1000).toLocaleTimeString()}</p>:null}
 {snapshot.phase==='EXERCISE_REQUIRED'?<p role="status">A little movement? Your camera stays off until you start.</p>:null}
 <div className="daily-actions"><button disabled={busy||!resting} onClick={()=>void act('reminder_action',{action:'start'})}>Start workout</button>{[5,15,30].map(n=><button key={n} disabled={busy||!resting} onClick={()=>void act('reminder_action',{action:'snooze',minutes:n})}>Snooze {n}m</button>)}<button disabled={busy||!resting} onClick={()=>void act('reminder_action',{action:'skip'})}>Skip break</button></div><small>{status?.sync}</small></div>
 <details><summary>Settings & routine</summary><form onSubmit={e=>{e.preventDefault();void act('save_daily_settings',{minutes,lockMode:lock});}}><label>Minutes between breaks <input type="number" min={1} max={240} value={minutes} onChange={e=>setMinutes(Number(e.target.value))}/></label><label><input type="checkbox" checked={lock} onChange={e=>setLock(e.target.checked)}/> Enforce lock mode (optional)</label><button disabled={busy}>Save settings</button></form><RoutineEditor/></details>
 <details><summary>Your recent workouts ({records.length})</summary>{records.length?records.map(r=><p key={r.id}>{r.date} · {r.exercise} · {r.reps?`${r.reps} reps`:`${r.seconds} seconds`} <button onClick={()=>void act('share_workout',{id:r.id})}>Review a public post ↗</button></p>):<p>Your completed workouts will appear here.</p>}</details>
 {error?<p role="alert">{error}</p>:null}</section>;
}
