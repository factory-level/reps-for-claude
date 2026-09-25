import {useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
type Routine={lifts?:{exercise:string;label?:string;sets:number;reps:number;weight?:number}[];conditioning?:{exercise:string;label?:string;rounds:number;seconds:number}[];stretches?:{name?:string;label?:string;seconds:number;perSide?:boolean}[]};
export function RoutineEditor(){
 const [routine,setRoutine]=useState<Routine|null>(null),[message,setMessage]=useState('');
 async function load(){try{setRoutine(JSON.parse(await invoke<string>('routine_settings')));setMessage('');}catch(e){setMessage(String(e));}}
 return <section><button onClick={()=>void load()}>Edit daily routine</button>{routine?<form className="routine-editor" onSubmit={e=>{e.preventDefault();void invoke('save_routine',{json:JSON.stringify(routine)}).then(()=>setMessage('Routine saved.')).catch(e=>setMessage(String(e)));}}>
 <p>Adjust your existing movements. Remove any you don’t want in your daily routine.</p>
 {(routine.lifts||[]).map((item,i)=><fieldset key={i}><legend>{item.label||item.exercise}</legend>{(['sets','reps','weight'] as const).map(key=><label key={key}>{key==='weight'?'Weight (lb)':key}<input type="number" min={key==='weight'?0:1} step={key==='weight'?'any':1} value={item[key]??0} onChange={e=>setRoutine({...routine,lifts:routine.lifts!.map((v,n)=>n===i?{...v,[key]:Number(e.target.value)}:v)})}/></label>)}<button type="button" onClick={()=>setRoutine({...routine,lifts:routine.lifts!.filter((_,n)=>n!==i)})}>Remove</button></fieldset>)}
 {(routine.conditioning||[]).map((item,i)=><fieldset key={i}><legend>{item.label||item.exercise}</legend>{(['rounds','seconds'] as const).map(key=><label key={key}>{key}<input type="number" min={1} value={item[key]} onChange={e=>setRoutine({...routine,conditioning:routine.conditioning!.map((v,n)=>n===i?{...v,[key]:Number(e.target.value)}:v)})}/></label>)}<button type="button" onClick={()=>setRoutine({...routine,conditioning:routine.conditioning!.filter((_,n)=>n!==i)})}>Remove</button></fieldset>)}
 {(routine.stretches||[]).map((item,i)=><fieldset key={i}><legend>{item.label||item.name||'Stretch'}</legend><label>Seconds<input type="number" min={1} value={item.seconds} onChange={e=>setRoutine({...routine,stretches:routine.stretches!.map((v,n)=>n===i?{...v,seconds:Number(e.target.value)}:v)})}/></label><button type="button" onClick={()=>setRoutine({...routine,stretches:routine.stretches!.filter((_,n)=>n!==i)})}>Remove</button></fieldset>)}
 <button>Save routine</button></form>:null}{message?<p role="status">{message}</p>:null}</section>;
}
