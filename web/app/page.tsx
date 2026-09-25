'use client';
import {useCallback,useEffect,useMemo,useRef,useState} from 'react';
import {progress,dateBefore} from '../lib/consistency';
import type {ConsistencyBoard,ConsistencyEntry} from '../lib/consistency';
class ApiError extends Error{constructor(message:string,public status:number){super(message);}}
async function api(path:string,method='GET',data?:unknown){
 const response=await fetch(`/api/${path}`,{method,cache:'no-store',headers:data?{'Content-Type':'application/json'}:undefined,body:data?JSON.stringify(data):undefined});
 const value=await response.json();if(!response.ok)throw new ApiError(value.error||'Please try again',response.status);return value;
}
function sampleProfile(index:number,end:string):ConsistencyEntry{
 const names=['Milo','Jun','Nia','Sol'];
 const days=Array.from({length:28},(_,i)=>({date:dateBefore(end,27-i),completed:(i+index)%6===0?0:Math.min(10,Math.floor(i/5)+3+(i+index)%4),target:10,updatedAt:end+'T12:00:00Z'}));
 return {id:`demo-${index}`,nickname:names[index],location:null,demo:true,...progress(days,end)};
}
function GuestCard({person}:{person:ConsistencyEntry}){
 const values=person.heatmap.filter(d=>d.ratio!==null),complete=values.filter(d=>d.ratio!>=1).length;
 return <article className="guest-card">
  <div className="guest-heading"><span className="avatar" aria-hidden="true">{person.nickname.slice(0,1).toUpperCase()}</span><div className="guest-name"><strong>{person.nickname}</strong>{person.location?<span>{person.location}</span>:null}</div>{person.demo?<span className="demo-tag">DEMO</span>:<span className="guest-tag">GUEST</span>}</div>
  <div className="guest-progress"><span>{values.length?<><b>{complete}</b> {complete===1?'routine':'routines'} completed</>:'A fresh start'}</span><span className={person.delta!==null&&person.delta>0?'delta up':'delta'} title="Change in average routine completion on tracked days: previous seven completed days versus the seven before. Untracked days are excluded.">{person.delta===null?'Just getting started':person.delta===0?'Keeping steady':`${person.delta>0?'↑':'↓'} ${Math.abs(person.delta)} pts`}</span></div>
  <div className="heatmap" aria-label={`${person.nickname}: routine completion over the last 28 days`}>{person.heatmap.map(day=><span key={day.date} tabIndex={0} className={`heat-cell ${day.ratio===null?'unknown':`level-${day.ratio===0?0:Math.min(4,Math.ceil(day.ratio*4))}`}`} title={`${day.date}: ${day.ratio===null?'not tracked':`${Math.round(day.ratio*100)}% of routine`}`} aria-label={`${day.date}: ${day.ratio===null?'not tracked':`${Math.round(day.ratio*100)}% of routine`}`}/>)}</div>
  <div className="guest-caption"><span>4 weeks of little wins</span><span>{person.latestPercent===null?'Routine tracking starts next sync':`${person.latestPercent}% of latest routine`}</span></div>
 </article>;
}
export default function Page(){
 const [board,setBoard]=useState<ConsistencyBoard|null>(null),[error,setError]=useState(''),[message,setMessage]=useState(''),[busy,setBusy]=useState(false);
 const [paused,setPaused]=useState(false),[engaged,setEngaged]=useState(false),[reduced,setReduced]=useState(false);
 const viewport=useRef<HTMLDivElement>(null),retryAt=useRef(0),inFlight=useRef(false);
 const refresh=useCallback(async()=>{
  if(inFlight.current||document.hidden||!navigator.onLine||Date.now()<retryAt.current)return;
  inFlight.current=true;
  try{setBoard(await api('consistency'));setError('');retryAt.current=0;}
  catch(e){retryAt.current=Date.now()+(e instanceof ApiError&&e.status===429?3600000:90000);setError('Taking a little breather. Updates will resume automatically.');}
  finally{inFlight.current=false;}
 },[]);
 useEffect(()=>{void refresh();const timer=setInterval(()=>void refresh(),90000);const resume=()=>void refresh();document.addEventListener('visibilitychange',resume);window.addEventListener('online',resume);return()=>{clearInterval(timer);document.removeEventListener('visibilitychange',resume);window.removeEventListener('online',resume);};},[refresh]);
 useEffect(()=>{const media=matchMedia('(prefers-reduced-motion: reduce)');const update=()=>setReduced(media.matches);update();media.addEventListener('change',update);return()=>media.removeEventListener('change',update);},[]);
 const people=useMemo(()=>{if(!board)return [];const real=board.entries;return [...real,...Array.from({length:Math.max(0,4-real.length)},(_,i)=>sampleProfile(i,board.endDate))];},[board]);
 useEffect(()=>{
  if(paused||engaged||reduced)return;
  const timer=setInterval(()=>{const box=viewport.current;if(!box||document.hidden)return;const step=(box.firstElementChild?.getBoundingClientRect().height??180)+14;if(box.scrollTop+box.clientHeight>=box.scrollHeight-4)box.scrollTo({top:0,behavior:'smooth'});else box.scrollBy({top:step,behavior:'smooth'});},5500);
  return()=>clearInterval(timer);
 },[paused,engaged,reduced,people.length]);
 return <main className="one-page">
  <section className="about-rfp" aria-labelledby="rfp-title">
   <a className="brand" href="/">RFP<span>↗</span></a><span className="eyebrow">CODE. WORKOUT. REPEAT.</span>
   <h1 id="rfp-title"><span className="mode-title code-title">REPS FOR</span><span className="mode-title workout-title">PROMPTS</span></h1>
   <div className="workout-code"><span className="workout-label"><svg className="mode-icon" viewBox="0 0 28 24" fill="none" aria-hidden="true" focusable="false"><path d="M8 10h12v4H8z" fill="var(--wood)"/><g fill="currentColor"><rect x="4" y="5" width="5" height="14" rx="1.5"/><rect x="19" y="5" width="5" height="14" rx="1.5"/><path d="M1 9h3v6H1zM24 9h3v6h-3z"/></g></svg>WORKOUT</span><span className="mode-divider">/</span><span className="code-label"><svg className="mode-icon" viewBox="0 0 28 24" fill="none" aria-hidden="true" focusable="false"><rect x="1" y="2" width="26" height="20" rx="3" fill="var(--sky)"/><path d="m7 8 4 4-4 4m8 0h6" strokeLinecap="round" strokeLinejoin="round"/></svg>CODE</span></div>
   <h2>While it thinks,<br/> you move.</h2><p>RFP spots when you’re using Claude Code or Codex and nudges you through your workout routine. A few reps, a stretch, then back to your prompts.</p><a className="github-link" href="https://github.com/factory-level/reps-for-claude" target="_blank" rel="noopener noreferrer">Get RFP on GitHub ↗</a>
   <form className="tiny-email" onSubmit={e=>{e.preventDefault();const form=e.currentTarget,data=new FormData(form);setBusy(true);setMessage('');void api('subscribe','POST',{email:data.get('email'),website:data.get('website')}).then(v=>{setMessage(v.message);form.reset();}).catch(e=>setMessage(e.message)).finally(()=>setBusy(false));}}><label htmlFor="email">A little RFP news, now and then?</label><div><input id="email" name="email" type="email" placeholder="you@example.com" required maxLength={254}/><button disabled={busy}>{busy?'…':'Keep me posted ↗'}</button></div><label className="honeypot" aria-hidden="true">Website<input name="website" tabIndex={-1}/></label><span role="status">{message||'Just occasional updates. Your email stays private.'}</span></form>
  </section>
  <section className="showcase" aria-labelledby="showcase-title">
   <div className="showcase-heading"><div><span className="eyebrow">A LITTLE, OFTEN.</span><h2 id="showcase-title">Keeping at it.</h2></div><button className="pause" onClick={()=>setPaused(v=>!v)} aria-pressed={paused} disabled={reduced}>{reduced?'Still view':paused?'Play ↓':'Pause Ⅱ'}</button></div>
   <p className="showcase-copy">People showing up for their own routines.</p>
   <div className="guest-scroll" ref={viewport} tabIndex={0} aria-label="Guest routine progress" onMouseEnter={()=>setEngaged(true)} onMouseLeave={()=>{if(!viewport.current?.contains(document.activeElement))setEngaged(false);}} onFocus={()=>setEngaged(true)} onBlur={e=>{if(!e.currentTarget.contains(e.relatedTarget))setEngaged(false);}}>
    {!board?<p className="empty-copy">{error||'Gathering the little wins…'}</p>:people.map(person=><GuestCard key={person.id} person={person}/>)}
   </div>
   <div className="heatmap-key"><span>Less of your routine</span><span className="key-cells" aria-hidden="true">{[0,1,2,3,4].map(n=><i key={n} className={`heat-cell level-${n}`}/>)}</span><span>All done</span></div>
   <p className="showcase-footnote">{people.some(p=>p.demo)?'Demo profiles keep the seats warm. Real guests replace them.':'Small steps, shared. No comparing routines.'}<br/>{error||'Refreshes automatically · dashed squares mean not tracked'}</p>
  </section>
  <DeleteLink/>
 </main>;
}
function DeleteLink(){
 const [link,setLink]=useState<{id:string;token:string}|null>(null),[error,setError]=useState('');
 useEffect(()=>{const id=new URLSearchParams(location.search).get('delete');if(id&&location.hash)setLink({id,token:location.hash.slice(1)});},[]);
 if(!link)return null;
 return <section className="notice"><p>Remove the workout associated with this private link?</p><button onClick={()=>void api(`activity/${link.id}`,'DELETE',{token:link.token}).then(()=>{setLink(null);history.replaceState(null,'','/');}).catch(e=>setError(e.message))}>Delete this workout</button>{error?<p role="alert">{error}</p>:null}</section>;
}
