/** Disposable end-to-end CLI test. TEST_DATABASE_URL must point at an isolated database. */
import {mkdtemp,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {execFileSync} from 'node:child_process';
import {randomBytes,randomUUID} from 'node:crypto';
import assert from 'node:assert/strict';
import postgres from 'postgres';
import {hash} from '../lib/api';
if(!process.env.TEST_DATABASE_URL)throw Error('TEST_DATABASE_URL required');
const sql=postgres(process.env.TEST_DATABASE_URL,{prepare:false});
const dir=await mkdtemp(join(tmpdir(),'rfp-cli-'));
const cli=process.env.REPS_TEST_CLI||'../app/src-tauri/target/release/reps';
const id=randomUUID(),read=randomBytes(32).toString('base64url'),upload=randomBytes(32).toString('base64url');
try{
 await sql`insert into reps.datasets(id,nickname,autopost,public_after) values(${id},'CLI test',true,now()-interval '1 hour')`;
 await sql`insert into reps.tokens(hash,dataset,scope) values(${hash(read)},${id},'read'),(${hash(upload)},${id},'upload')`;
 for(const [file,token] of [['remote.json',read],['upload.json',upload]])await writeFile(join(dir,file),JSON.stringify({url:'http://127.0.0.1:3017',token}),{mode:0o600});
 const run=(...args:string[])=>JSON.parse(execFileSync(cli,[...args,'--json'],{env:{...process.env,REPS_APP_HOME:dir},encoding:'utf8'}));
 assert.equal(run('sync').synced,0);
 execFileSync('python3',['-c',`import sqlite3,sys,datetime
with sqlite3.connect(sys.argv[1]) as db:
 db.execute("INSERT INTO exercise_history(date,exercise,kind,reps,seconds,weight,verified,public_id,recorded_at,weight_unit) VALUES ('2026-09-24','squat','rep',10,0,0,0,'cli-set',?,'lb')",(datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00','Z'),))`,join(dir,'reps.sqlite')]);
 assert.equal(run('history').records.length,1);
 assert.equal(run('sync').synced,1);assert.equal(run('sync').synced,0);
 assert.equal(run('history','--source','remote').records[0].id,'cli-set');
 assert.equal(run('summary','--source','remote').reps,10);
 assert.equal((await sql`select * from reps.activities where source_key=${id+':cli-set'}`).length,1);
 console.log('CLI E2E PASS: local record → HTTP sync → one public post → remote agent read; retry deduplicated.');
}finally{await sql`delete from reps.activities where source_key like ${id+':%'}`;await sql`delete from reps.workouts where dataset=${id}`;await sql`delete from reps.tokens where dataset=${id}`;await sql`delete from reps.datasets where id=${id}`;await sql.end();await rm(dir,{recursive:true});}
