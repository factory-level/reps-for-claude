-- RFP server only. Password is provisioned separately, never committed.
-- tokens contains SHA-256 hashes and scopes, never plaintext credentials.
do $$ begin
 create role rfp_web login nosuperuser nocreatedb nocreaterole noinherit nobypassrls;
exception when duplicate_object then null;
end $$;
grant usage on schema reps to rfp_web;
grant select on all tables in schema reps to rfp_web;
grant insert on reps.activities,reps.cheers,reps.reports,reps.subscribers,reps.rate_limits,reps.workouts to rfp_web;
grant delete on reps.activities to rfp_web;
grant update on reps.rate_limits to rfp_web;
grant update(synced_at) on reps.datasets to rfp_web;
grant update(posted) on reps.workouts to rfp_web;
grant usage on all sequences in schema reps to rfp_web;
grant execute on function reps.consume_budget(), reps.limit_storage() to rfp_web;
do $$
declare t text;
begin
 foreach t in array array['activities','cheers','reports','subscribers','rate_limits','datasets','tokens','workouts'] loop
  execute format('drop policy if exists server_access on reps.%I',t);
  execute format('create policy server_access on reps.%I to rfp_web using (true) with check (true)',t);
 end loop;
end $$;
