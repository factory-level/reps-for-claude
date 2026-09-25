/** Real CLI → HTTP → isolated PostgreSQL, with two independently authenticated sites. */
import {mkdtemp,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {createServer} from 'node:http';
import {randomBytes,randomUUID} from 'node:crypto';
import assert from 'node:assert/strict';
import {handle,hash} from '../lib/api';
import {db} from '../lib/db';
if(!process.env.TEST_DATABASE_URL)throw Error('Use an isolated TEST_DATABASE_URL');
process.env.DATABASE_URL=process.env.TEST_DATABASE_URL;process.env.RATE_LIMIT_SECRET='isolated-profile-test';
const sql=db(),exec=promisify(execFile),dir=await mkdtemp(join(tmpdir(),'rfp-sites-'));
const cli=process.env.REPS_TEST_CLI||'../app/src-tauri/target/release/reps';
const run=async(...args:string[])=>JSON.parse((await exec(cli,[...args,'--json'],{env:{...process.env,REPS_APP_HOME:dir}})).stdout);
const servers:ReturnType<typeof createServer>[]=[],ids:string[]=[];
try{
 await sql`delete from reps.rate_limits`;
 for(let i=0;i<2;i++){
  const id=randomUUID(),token=randomBytes(32).toString('base64url');ids.push(id);
  await sql`insert into reps.datasets(id) values(${id})`;
  await sql`insert into reps.tokens(hash,dataset,scope) values(${hash(token)},${id},'upload')`;
  const file=join(dir,`token-${i}`);await writeFile(file,token,{mode:0o600});
  const server=createServer(async(req,res)=>{
   try{const chunks:Buffer[]=[];for await(const chunk of req)chunks.push(chunk);
    const response=await handle(new Request(`http://127.0.0.1${req.url}`,{method:req.method,headers:req.headers as Record<string,string>,body:['GET','HEAD'].includes(req.method!)?undefined:Buffer.concat(chunks)}));
    res.writeHead(response.status,Object.fromEntries(response.headers));res.end(Buffer.from(await response.arrayBuffer()));
   }catch{res.writeHead(500);res.end();}
  });servers.push(server);
  await new Promise<void>(resolve=>server.listen(0,'127.0.0.1',resolve));
  const port=(server.address() as {port:number}).port,url=`http://127.0.0.1:${port}`;
  await run('site','--url',url,'--upload-token-file',file);
  const profile=await run('profile','--nickname',`CLI person ${i}`,'--location',`Test City ${i}`);assert.equal(profile.sharing,true);assert.equal(profile.location,`Test City ${i}`);
 }
 const url=(i:number)=>`http://127.0.0.1:${(servers[i].address() as {port:number}).port}`;
 await run('site','--url',url(0));assert.equal((await run('sync')).synced,0);
 await exec('python3',['-c',`import sqlite3,sys,datetime
with sqlite3.connect(sys.argv[1]) as db:
 db.execute("INSERT INTO exercise_history(date,exercise,kind,reps,seconds,weight,verified,public_id,recorded_at,weight_unit) VALUES ('2026-09-25','squat','rep',5,0,45,1,'profile-cli-set',?,'lb')",(datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00','Z'),))`,join(dir,'reps.sqlite')]);
 assert.equal((await run('sync')).synced,1);assert.equal((await run('sync')).synced,0);
 await run('site','--url',url(1));assert.equal((await run('sync')).synced,1);
 for(let i=0;i<2;i++){const rows=await sql`select nickname,location from reps.activities where source_key=${ids[i]+':profile-cli-set'}`;assert.equal(rows.length,1);assert.equal(rows[0].nickname,`CLI person ${i}`);assert.equal(rows[0].location,`Test City ${i}`);}
 await run('site','--url',url(0));assert.equal((await run('sync')).synced,0);
 assert.equal((await run('profile','--clear-location')).location,null);assert.equal((await sql`select location from reps.activities where source_key=${ids[0]+':profile-cli-set'}`)[0].location,null);
 console.log('PASS: default sharing, CLI nickname, authenticated endpoint switching, independent upload queues, and idempotent public posts across two sites.');
}finally{
 for(const s of servers)await new Promise<void>(resolve=>s.close(()=>resolve()));
 for(const id of ids){await sql`delete from reps.activities where source_key like ${id+':%'}`;await sql`delete from reps.workouts where dataset=${id}`;await sql`delete from reps.tokens where dataset=${id}`;await sql`delete from reps.datasets where id=${id}`;}
 await sql.end();await rm(dir,{recursive:true,force:true});
}
