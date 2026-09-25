-- Fixed budget rows: requests cannot create unbounded counter storage.
create or replace function reps.consume_budget() returns boolean
language plpgsql set search_path = '' as $$
declare n integer; window_seconds integer; ceiling integer; period bigint;
begin
 foreach window_seconds in array array[86400,2592000] loop
  ceiling := case when window_seconds=86400 then 2000 else 40000 end;
  period := floor(extract(epoch from now()) / window_seconds);
  insert into reps.rate_limits(key,bucket,count)
   values ('budget:'||window_seconds,period,1)
   on conflict(key) do update set bucket=excluded.bucket,
    count=case when reps.rate_limits.bucket=excluded.bucket then least(reps.rate_limits.count+1,ceiling+1) else 1 end
   returning count into n;
  if n>ceiling then return false; end if;
 end loop;
 return true;
end $$;
revoke all on function reps.consume_budget() from public;

-- Serialize inserts before counting so concurrent clients cannot exceed caps.
-- Local history is never deleted; cloud writes pause when full.
create or replace function reps.limit_storage() returns trigger
language plpgsql set search_path = '' as $$
declare n bigint;
begin
 perform pg_advisory_xact_lock(hashtextextended('reps.storage.'||TG_TABLE_NAME,0));
 execute format('select count(*) from %I.%I',TG_TABLE_SCHEMA,TG_TABLE_NAME) into n;
 if n>=TG_ARGV[0]::integer then
  -- Allow idempotent retries at capacity.
  if TG_TABLE_NAME='workouts' then
   if exists(select 1 from reps.workouts where dataset=NEW.dataset and id=NEW.id) then return NEW; end if;
  elsif TG_TABLE_NAME='activities' then
   if exists(select 1 from reps.activities where source_key=NEW.source_key) then return NEW; end if;
  elsif TG_TABLE_NAME='subscribers' then
   if exists(select 1 from reps.subscribers where email=NEW.email) then return NEW; end if;
  elsif TG_TABLE_NAME='cheers' then
   if exists(select 1 from reps.cheers where activity_id=NEW.activity_id and visitor_hash=NEW.visitor_hash) then return NEW; end if;
  elsif TG_TABLE_NAME='reports' then
   if exists(select 1 from reps.reports where activity_id=NEW.activity_id and visitor_hash=NEW.visitor_hash) then return NEW; end if;
  end if;
  raise exception 'RFP cloud storage limit reached';
 end if;
 return NEW;
end $$;
revoke all on function reps.limit_storage() from public;
do $$
declare t text; cap integer;
begin
 for t,cap in select * from (values ('activities',20000),('workouts',20000),('subscribers',5000),('cheers',50000),('reports',10000)) as caps(t,c) loop
  execute format('drop trigger if exists storage_cap on reps.%I',t);
  execute format('create trigger storage_cap before insert on reps.%I for each row execute function reps.limit_storage(%L)',t,cap);
 end loop;
end $$;
