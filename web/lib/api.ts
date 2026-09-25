import { progress, mergeDays } from './consistency';
import { createHash, createHmac, randomBytes } from 'node:crypto';
import { ZodError } from 'zod';
import { db } from './db';
import { subscriber, syncBody, query, profileUpdate } from './validation';
export const hash=(s:string)=>createHash('sha256').update(s).digest('hex');
class HttpError extends Error { constructor(public status:number, message:string){ super(message); } }
const json=(v:unknown,status=200)=>Response.json(v,{status,headers:{'Cache-Control':'no-store',...(status===429?{'Retry-After':'3600'}:{})}});
async function budget() {
 const rows=await db()`select reps.consume_budget() allowed`;
 if(!rows[0].allowed)throw new HttpError(429,'Community usage limit reached. Workouts stay saved locally; try again later.');
}
async function body(req:Request) {
 if (!req.headers.get('content-type')?.startsWith('application/json')) throw new HttpError(415,'Use application/json');
 const reader=req.body?.getReader(); if (!reader) throw new HttpError(400,'Missing body');
 const chunks:Uint8Array[]=[]; let length=0;
 while(true){ const r=await reader.read(); if(r.done)break; length+=r.value.length; if(length>100000){await reader.cancel();throw new HttpError(413,'Request too large');} chunks.push(r.value); }
 try{return JSON.parse(Buffer.concat(chunks).toString());}catch{throw new HttpError(400,'Invalid JSON');}
}
async function visitor(req:Request, action:string, max:number) {
 const origin=req.headers.get('origin');
 if(origin){
  let originUrl:URL;try{originUrl=new URL(origin);}catch{throw new HttpError(403,'Invalid origin');}
  const host=req.headers.get('host')||new URL(req.url).host;
  if(originUrl.host!==host || (process.env.VERCEL && originUrl.protocol!=='https:'))throw new HttpError(403,'Cross-site request rejected');
 }
 const secret=process.env.RATE_LIMIT_SECRET;
 if(!secret)throw new HttpError(503,'Posting is not configured yet');
 // Vercel overwrites this trusted ingress header. Locally all visitors share a bucket.
 const address=process.env.VERCEL ? req.headers.get('x-vercel-forwarded-for') || 'unknown' : 'local';
 const key=createHmac('sha256',secret).update(address).digest('hex');
 const bucket=Math.floor(Date.now()/3600000); const sql=db();
 const rows=await sql`insert into reps.rate_limits(key,bucket,count) values(${action+key.slice(0,2)},${bucket},1)
 on conflict(key) do update set bucket=excluded.bucket,count=case when reps.rate_limits.bucket=excluded.bucket then reps.rate_limits.count+1 else 1 end returning count`;
 if(rows[0].count>max) throw new HttpError(429,'Take a breather and try again later');
 return key;
}
async function credential(req:Request,scope:string){
 const token=req.headers.get('authorization')?.match(/^Bearer ([A-Za-z0-9_-]{32,})$/)?.[1];
 if(!token)throw new HttpError(401,'A bearer token is required');
 const rows=await db()`select dataset,scope from reps.tokens where hash=${hash(token)}`;
 if(!rows.length)throw new HttpError(401,'Invalid or revoked token');
 if(rows[0].scope!==scope)throw new HttpError(403,`Requires ${scope} credential`);
 const bucket=Math.floor(Date.now()/60000),key='credential:'+hash(token);
 const counts=await db()`insert into reps.rate_limits(key,bucket,count) values(${key},${bucket},1) on conflict(key) do update set bucket=excluded.bucket,count=case when reps.rate_limits.bucket=excluded.bucket then reps.rate_limits.count+1 else 1 end returning count`;
 if(counts[0].count > (scope==='upload'?12:120))throw new HttpError(429,'Too many requests; retry later');
 return String(rows[0].dataset);
}
export async function handle(req:Request):Promise<Response>{
 try{
 const url=new URL(req.url), path=url.pathname.replace(/^\/api\//,'');
 if(path==='activity' && req.method==='POST')return Response.json({error:'Workouts are posted automatically by RFP.'},{status:405,headers:{Allow:'GET','Cache-Control':'no-store'}});
 if(process.env.DATABASE_URL)await budget();
 if(path==='consistency' && req.method==='GET'){
  const endDate=new Date().toISOString().slice(0,10);
  if(!process.env.DATABASE_URL)return json({entries:[],endDate});
  const rows=await db()`select id,nickname,location,routine_days,public_after from reps.datasets
   where autopost and public_after is not null and (jsonb_array_length(routine_days)>0 or exists(select 1 from reps.activities a where left(a.source_key,length(reps.datasets.id)+1)=reps.datasets.id||':'))
   order by synced_at desc nulls last,id limit 24`;
  return json({entries:rows.map(r=>({id:hash(String(r.id)).slice(0,16),nickname:r.nickname,location:r.location,
   ...progress(r.routine_days.filter((d:{updatedAt:string})=>new Date(d.updatedAt)>=new Date(r.public_after)),endDate)})),endDate});
 }
 if(path==='activity' && req.method==='GET'){
  if(!process.env.DATABASE_URL)return json({items:[],featured:[],nextCursor:null,configured:false});
  const q=query(url),sql=db();
  const [rows,featured]=await Promise.all([
   sql`select a.id::text,a.nickname,a.location,a.exercise,a.quantity::float8,a.unit,a.note,a.created_at,a.featured,(select count(*)::int from reps.cheers c where c.activity_id=a.id) cheers from reps.activities a where (${q.cursor}=0 or a.id<${q.cursor}) order by a.id desc limit ${q.limit+1}`,
   sql`select a.id::text,a.nickname,a.location,a.exercise,a.quantity::float8,a.unit,a.note,a.created_at,a.featured,(select count(*)::int from reps.cheers c where c.activity_id=a.id) cheers from reps.activities a where a.featured order by a.id desc limit 3`
  ]);
  return Response.json({items:rows.slice(0,q.limit),featured,nextCursor:rows.length>q.limit?rows[q.limit-1].id:null,configured:true},{headers:{'Cache-Control':'no-store'}});
 }
 const match=path.match(/^activity\/(\d+)(?:\/(cheer|report))?$/);
 if(match){
  const [,id,action]=match;
  if(req.method==='DELETE'&&!action){
   await visitor(req,'delete',30); const v=await body(req);
   if(typeof v.token!=='string')throw new HttpError(400,'Deletion token required');
   const rows=await db()`delete from reps.activities where id=${id} and deletion_hash=${hash(v.token)} returning id`;
   if(!rows.length)throw new HttpError(404,'Post or deletion token not found'); return json({ok:true});
  }
  if(req.method==='POST'&&action){
   const who=await visitor(req,action,100),sql=db();
   const exists=await sql`select id from reps.activities where id=${id}`;if(!exists.length)throw new HttpError(404,'Post not found');
   if(action==='cheer')await sql`insert into reps.cheers values(${id},${who}) on conflict do nothing`;
   else await sql`insert into reps.reports(activity_id,visitor_hash) values(${id},${who}) on conflict do nothing`;
   return json({ok:true});
  }
 }
 if(path==='subscribe'&&req.method==='POST'){
  await visitor(req,'subscribe',5);const v=subscriber.parse(await body(req));
  await db()`insert into reps.subscribers(email) values(${v.email}) on conflict do nothing`;
  return json({message:'Thanks! Your email is on the updates list.'});
 }
 if(path==='v1/profile' && ['GET','POST'].includes(req.method)){
  const dataset=await credential(req,'upload');
  if(req.method==='POST'){
   const v=profileUpdate.parse(await body(req));
   await db().begin(async sql=>{
   await sql`update reps.datasets set nickname=coalesce(${v.nickname??null},nickname),
    public_after=case when ${v.sharing===true} and (not autopost or public_after is null) then now() else public_after end,
    autopost=coalesce(${v.sharing??null},autopost),
    location=case when ${v.location!==undefined} then ${v.location??null} else location end where id=${dataset}`;
   // Row update serializes with sync's dataset lock, so an in-flight post cannot restore a cleared label.
   if(v.location===null)await sql`update reps.activities set location=null where left(source_key,length(${dataset})+1)=${dataset+':'} and location is not null`;
   });
  }
  const [p]=await db()`select nickname,autopost,public_after,synced_at,location from reps.datasets where id=${dataset}`;
  return json({schemaVersion:1,nickname:p.nickname,location:p.location,sharing:p.autopost,publicAfter:p.public_after,syncedAt:p.synced_at,automaticUpload:true});
 }
 if(path==='v1/sync'&&req.method==='POST'){
  const dataset=await credential(req,'upload'),v=syncBody.parse(await body(req));
  await db().begin(async sql=>{
   for(const r of v.records) await sql`insert into reps.workouts(dataset,id,date,record) values(${dataset},${r.id},${r.date},${sql.json(r)}) on conflict(dataset,id) do nothing`;
   // Serialize each dataset's publications, including concurrent retries.
   const [owner]=await sql`select * from reps.datasets where id=${dataset} for update`;
   if(v.routineDays.length){const days=mergeDays(owner.routine_days??[],v.routineDays,new Date().toISOString().slice(0,10));await sql`update reps.datasets set routine_days=${sql.json(days)} where id=${dataset}`;}
   if(owner.autopost && owner.public_after){
    const recent=await sql`select count(*)::int n from reps.activities where source_key like ${dataset+':%'} and created_at>now()-interval '1 hour'`;
    const slots=Math.max(0,6-recent[0].n);
    const pending=await sql`select id,record from reps.workouts where dataset=${dataset} and not posted and (record->>'recordedAt')::timestamptz>=${owner.public_after} order by sequence asc limit ${slots}`;
    for(const row of pending){
     const r=row.record;
     await sql`insert into reps.activities(nickname,exercise,quantity,unit,note,deletion_hash,source_key,location)
      values(${owner.nickname},${r.exercise},${r.kind==='REP'?r.reps:r.seconds},${r.kind==='REP'?'reps':'seconds'},${'A little movement between prompts.'},${hash(randomBytes(32).toString('hex'))},${dataset+':'+row.id},${owner.location??null}) on conflict(source_key) do nothing`;
     await sql`update reps.workouts set posted=true where dataset=${dataset} and id=${row.id}`;
    }
   }
   await sql`update reps.datasets set synced_at=now() where id=${dataset}`;
  }); return json({schemaVersion:1,accepted:v.records.map(r=>r.id)});
 }
 if(['v1/history','v1/summary','v1/status'].includes(path)&&req.method==='GET'){
  const dataset=await credential(req,'read'),q=query(url),sql=db();
  const [state]=await sql`select synced_at from reps.datasets where id=${dataset}`;
  const base={schemaVersion:1,source:'remote',syncedAt:state.synced_at};
  if(path==='v1/status')return json(base);
  if(path==='v1/summary'){
   const rows=await sql`select count(*)::int sets,coalesce(sum((record->>'reps')::numeric),0)::float8 reps,coalesce(sum((record->>'seconds')::numeric),0)::float8 seconds from reps.workouts where dataset=${dataset} and (${q.from??null}::text is null or date>=${q.from??null}) and (${q.to??null}::text is null or date<=${q.to??null})`;
   return json({...base,...rows[0]});
  }
  const rows=await sql`select sequence,record from reps.workouts where dataset=${dataset} and (${q.cursor}=0 or sequence<${q.cursor}) and (${q.from??null}::text is null or date>=${q.from??null}) and (${q.to??null}::text is null or date<=${q.to??null}) order by sequence desc limit ${q.limit+1}`;
  return json({...base,records:rows.slice(0,q.limit).map(r=>r.record),nextCursor:rows.length>q.limit?String(rows[q.limit-1].sequence):null});
 }
 return json({error:'Not found'},404);
 }catch(error){
  if(error instanceof ZodError)return json({error:'Check the submitted fields',details:error.issues.map(i=>i.message)},400);
  if(error instanceof HttpError)return json({error:error.message},error.status);
  if(error && typeof error==='object' && 'code' in error && error.code==='P0001')return json({error:'Cloud storage limit reached. Workouts remain saved locally.'},429);
  // Do not log database URLs, statements, tokens or personal data.
  console.error('API request failed',error instanceof Error?error.name:'UnknownError');
  return json({error:'Temporarily unavailable. Please try again.'},503);
 }
}
