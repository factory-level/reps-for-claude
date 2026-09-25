-- Bounded per-guest routine snapshots, not a competitive score or identity system.
alter table reps.datasets add column if not exists routine_days jsonb not null default '[]'::jsonb check(jsonb_typeof(routine_days)='array' and jsonb_array_length(routine_days)<=28);
grant update(routine_days) on reps.datasets to rfp_web;
