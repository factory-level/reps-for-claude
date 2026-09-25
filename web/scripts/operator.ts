import { readFile } from 'node:fs/promises';
import { randomBytes,randomUUID } from 'node:crypto';
import { db } from '../lib/db';
import { hash } from '../lib/api';
const [command,arg,scope]=process.argv.slice(2),sql=db();
try{
 switch(command){
 case 'migrate':await sql.unsafe(await readFile(new URL('../db/001_activity.sql',import.meta.url),'utf8'));await sql.unsafe(await readFile(new URL('../supabase/migrations/20260925175039_rfp_usage_guards.sql',import.meta.url),'utf8'));break;
 case 'usage':console.log(JSON.stringify({budgets:await sql`select key,bucket,count from reps.rate_limits where key like 'budget:%'`,storage:await sql`select pg_size_pretty(pg_database_size(current_database())) database_size`},null,2));break;
 case 'dataset': {const id=randomUUID();await sql`insert into reps.datasets(id) values(${id})`;console.log(id);break;}
 case 'token': {
  if(!arg||!['read','upload'].includes(scope))throw Error('token DATASET read|upload');
  const token=randomBytes(32).toString('base64url');await sql`insert into reps.tokens(hash,dataset,scope) values(${hash(token)},${arg},${scope})`;
  console.log(JSON.stringify({token,tokenId:hash(token),scope}));break;
 }
 case 'autopost':if(!arg||!scope||scope.length>32)throw Error('autopost DATASET NICKNAME');await sql`update reps.datasets set autopost=true,nickname=${scope},public_after=coalesce(public_after,now()) where id=${arg}`;break;
 case 'stop-posting':await sql`update reps.datasets set autopost=false where id=${arg}`;break;
 case 'revoke':await sql`delete from reps.tokens where hash=${arg}`;break;
 case 'feature':await sql`update reps.activities set featured=true where id=${arg}`;break;
 case 'unfeature':await sql`update reps.activities set featured=false where id=${arg}`;break;
 case 'remove':await sql`delete from reps.activities where id=${arg}`;break;
 case 'reports':console.log(JSON.stringify(await sql`select a.id,a.nickname,a.exercise,a.note,count(*) reports from reps.reports r join reps.activities a on a.id=r.activity_id group by a.id order by reports desc`,null,2));break;
 case 'unsubscribe':await sql`delete from reps.subscribers where email=${arg?.toLowerCase()}`;break;
 case 'subscribers':{
  const escape=(s:string)=>'"'+(/^[=+@-]/.test(s)?"'":'')+s.replaceAll('"','""')+'"';
  console.log('email,created_at');for(const r of await sql`select email,created_at::text from reps.subscribers order by created_at`)console.log([r.email,r.created_at].map(escape).join(','));break;
 }
 default:throw Error('Commands: migrate, dataset, token DATASET read|upload, revoke HASH, feature ID, unfeature ID, remove ID, reports, subscribers, unsubscribe EMAIL');
 }
} catch(error){console.error(error instanceof Error?error.message:'Operation failed');process.exitCode=1;}finally{await sql.end();}
