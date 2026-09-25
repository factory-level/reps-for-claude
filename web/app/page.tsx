'use client';
import { useCallback, useEffect, useState } from 'react';
import type { Activity } from '../lib/validation';
async function api(path:string, method='GET', data?:unknown){
 const res=await fetch(`/api/${path}`,{method,headers:data?{'Content-Type':'application/json'}:undefined,body:data?JSON.stringify(data):undefined});
 const v=await res.json();if(!res.ok)throw Error(v.error||'Please try again');return v;
}
export default function Page(){
 const [items,setItems]=useState<Activity[]>([]),[featured,setFeatured]=useState<Activity[]>([]),[cursor,setCursor]=useState<string|null>(null);
 const [error,setError]=useState(''),[message,setMessage]=useState(''),[busy,setBusy]=useState(false),[ready,setReady]=useState(false),[configured,setConfigured]=useState(true);
 const [tokens,setTokens]=useState<Record<string,string>>({});
 const [exercise,setExercise]=useState(''),[quantity,setQuantity]=useState('10'),[unit,setUnit]=useState('reps');
 const refresh=useCallback(async()=>{const v=await api('activity');setItems(v.items);setFeatured(v.featured);setCursor(v.nextCursor);setConfigured(v.configured);setReady(true);},[]);
 useEffect(()=>{
  void refresh().catch(e=>setError(e.message));
  try{setTokens(JSON.parse(localStorage.getItem('rfp-deletions')||'{}'));}catch{}
  const p=new URLSearchParams(location.search);setExercise(p.get('exercise')||'');setQuantity(p.get('quantity')||'10');
  const u=p.get('unit');if(u&&['reps','seconds','minutes'].includes(u))setUnit(u);
 },[refresh]);
 async function act(fn:()=>Promise<void>){setBusy(true);setError('');setMessage('');try{await fn();}catch(e){setError(e instanceof Error?e.message:'Please try again');}finally{setBusy(false);}}
 function card(a:Activity,spotlight=false){return <article className={spotlight?'card spotlight':'card'} key={a.id}>
  <div className="avatar" aria-hidden>{a.nickname.slice(0,1).toUpperCase()}</div><div className="activity-content">
   <div className="byline"><strong>{a.nickname}</strong><time dateTime={a.created_at}>{new Date(a.created_at).toLocaleDateString(undefined,{month:'short',day:'numeric'})}</time></div>
   <p className="workout">{a.quantity} {a.unit} <span>of {a.exercise}</span></p>{a.note?<p className="note">{a.note}</p>:null}
   <div className="actions"><button disabled={busy} aria-label={`Cheer ${a.nickname}`} onClick={()=>void act(async()=>{await api(`activity/${a.id}/cheer`,'POST');await refresh();})}>👏 {a.cheers || 'Cheer'}</button>
    <span className="self-reported">Self-reported</span>
    <button className="quiet" disabled={busy} onClick={()=>void act(async()=>{await api(`activity/${a.id}/report`,'POST');setMessage('Reported. Thanks for looking out for the community.');})}>Report</button>
    {tokens[a.id]?<button className="quiet" disabled={busy} onClick={()=>void act(async()=>{await api(`activity/${a.id}`,'DELETE',{token:tokens[a.id]});const next={...tokens};delete next[a.id];setTokens(next);localStorage.setItem('rfp-deletions',JSON.stringify(next));await refresh();})}>Delete mine</button>:null}
   </div></div></article>;}
 return <><header><a className="brand" href="/">RFP<span>↗</span></a><span className="wordmark">reps for prompts</span><a className="header-link" href="#post">Add your workout <span>＋</span></a></header>
 <main><section className="hero"><div className="hero-copy"><div className="eyebrow"><span className="dot"/> SMALL BREAKS. GOOD COMPANY.</div><h1><span className="mode-title code-title">CODE.</span><span className="mode-title workout-title">WORKOUT.</span></h1><p>While it thinks, you move.<br/>Little workouts between prompts, together.</p><a className="primary" href="#post">I moved. Let me post it ↗</a><span className="no-account">No accounts. No leaderboards. Just showing up.</span></div><div className="hero-art" aria-hidden="true"><div className="code-window"><div className="window-bar"><i/><i/><i/><span>ONE MORE PROMPT</span></div><div className="terminal-glyph">&gt;_<span className="pixel-lines"/></div><div className="window-caption">A little focus.</div></div><div className="workout-window"><div className="dumbbell"><i/><b/><i/></div><strong>+1 REP</strong><span>A little movement.</span></div><div className="logged-label">✓ ALL COUNTS.</div></div></section>
 <div className="community-bar"><span>THE BREAK ROOM</span><span>Code a little. Move a little. Cheer each other on.</span></div>
 {featured.length?<section className="featured"><div className="section-heading"><h2>A little spotlight</h2><span>People getting a little movement in</span></div><div className="featured-grid">{featured.map(a=>card(a,true))}</div></section>:null}
 <div className="columns"><section><div className="section-heading"><h2>The activity</h2><span>Every little bit counts ↘</span></div>
 {items.map(a=>card(a))}{ready&&!items.length?<div className="empty"><span className="empty-icon" aria-hidden="true">+1</span><h3>Somebody has to go first.</h3><p>Your ten squats belong here. So does your two-minute stretch.</p><a href="#post">Add the first workout ↗</a></div>:null}
 {!ready&&!error?<p>Getting the latest movement…</p>:null}
 {cursor?<button disabled={busy} onClick={()=>void act(async()=>{const v=await api(`activity?cursor=${cursor}`);setItems(old=>[...old,...v.items]);setCursor(v.nextCursor);})}>More movement ↓</button>:null}
 </section><aside><section className="composer" id="post"><div className="eyebrow">YOUR TURN</div><h2>What did you do?</h2><p>Big effort, tiny effort. It all belongs.</p>
 <form onSubmit={e=>{e.preventDefault();const form=e.currentTarget;const f=new FormData(form);void act(async()=>{
  const v=await api('activity','POST',{nickname:f.get('nickname'),exercise,quantity:Number(quantity),unit,note:f.get('note'),website:f.get('website')});
  const next={...tokens,[v.id]:v.deletionToken};setTokens(next);try{localStorage.setItem('rfp-deletions',JSON.stringify(next));}catch{setMessage('Posted. Save your deletion link below; browser storage is unavailable.');}
  setMessage(`Posted! Keep your deletion link: ${location.origin}/?delete=${v.id}#${v.deletionToken}`);form.reset();setExercise('');setQuantity('10');await refresh();
 });}}>
 <label>Your nickname<input name="nickname" placeholder="Call me…" required maxLength={32}/></label>
 <label>Movement<input value={exercise} onChange={e=>setExercise(e.target.value)} placeholder="Squats, stretches, kitchen dancing…" required maxLength={60}/></label>
 <div className="quantity"><label>How much?<input type="number" min={unit==='reps'?1:0.1} max="100000" step={unit==='reps'?1:'any'} value={quantity} onChange={e=>setQuantity(e.target.value)} required/></label><label>Unit<select value={unit} onChange={e=>setUnit(e.target.value)}><option value="reps">reps</option><option value="seconds">seconds</option><option value="minutes">minutes</option></select></label></div>
 <label>A little note <span>(optional)</span><textarea name="note" maxLength={280} placeholder="The agent is still thinking…" rows={2}/></label>
 <label className="honeypot" aria-hidden>Website<input name="website" tabIndex={-1} autoComplete="off"/></label>
 <button className="primary" disabled={busy||!configured}>Post my movement ↗</button><small>Public, under your nickname. Save your deletion link to remove it later. Nicknames aren’t verified.</small>
 </form></section><section className="email"><h3>Stay in the loop.</h3><p>Occasional RFP updates. Your email stays private.</p><form onSubmit={e=>{e.preventDefault();const form=e.currentTarget;const f=new FormData(form);void act(async()=>{const v=await api('subscribe','POST',{email:f.get('email'),website:f.get('website')});setMessage(v.message);form.reset();});}}><label>Email<input name="email" type="email" placeholder="you@example.com" required maxLength={254}/></label><label className="honeypot" aria-hidden>Website<input name="website" tabIndex={-1}/></label><button disabled={busy||!configured}>Email me updates ↗</button><small>Optional. Posting doesn’t subscribe you.</small></form></section></aside></div>
 {!configured?<p className="notice">RFP is getting ready. Posting will open once the activity database is connected.</p>:null}
 <div className="feedback" aria-live="polite">{error?<p role="alert">{error}</p>:null}{message?<p>{message}</p>:null}{error||message?<button onClick={()=>{setError('');setMessage('');}}>Dismiss</button>:null}</div>
 <DeleteLink onDelete={refresh}/>
 </main><footer><strong>RFP ↗</strong><span>Made for humans waiting on machines.</span><span>A little movement is enough.</span></footer></>;
}
function DeleteLink({onDelete}:{onDelete:()=>Promise<void>}){
 const [link,setLink]=useState<{id:string;token:string}|null>(null),[error,setError]=useState('');
 useEffect(()=>{const id=new URLSearchParams(location.search).get('delete');if(id&&location.hash)setLink({id,token:location.hash.slice(1)});},[]);
 if(!link)return null;
 return <section className="notice"><p>Remove the workout associated with this private link?</p><button onClick={()=>void api(`activity/${link.id}`,'DELETE',{token:link.token}).then(async()=>{setLink(null);history.replaceState(null,'','/');await onDelete();}).catch(e=>setError(e.message))}>Delete this workout</button>{error?<p role="alert">{error}</p>:null}</section>;
}
