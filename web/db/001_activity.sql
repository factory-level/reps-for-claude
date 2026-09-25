-- Private schema is not exposed by Supabase's Data API.
create schema if not exists reps;
revoke all on schema reps from public;
create table if not exists reps.activities (
 id bigint generated always as identity primary key,
 nickname text not null check(length(nickname) between 1 and 32),
 exercise text not null check(length(exercise) between 1 and 60),
 quantity numeric not null check(quantity > 0 and quantity <= 100000),
 unit text not null check(unit in ('reps','seconds','minutes')),
 note text not null default '' check(length(note) <= 280),
 created_at timestamptz not null default now(),
 deletion_hash text not null,
 featured boolean not null default false
);
create table if not exists reps.cheers (
 activity_id bigint references reps.activities on delete cascade,
 visitor_hash text not null,
 primary key(activity_id,visitor_hash)
);
create table if not exists reps.reports (
 activity_id bigint references reps.activities on delete cascade,
 visitor_hash text not null, created_at timestamptz not null default now(),
 primary key(activity_id,visitor_hash)
);
create table if not exists reps.subscribers (
 email text primary key, created_at timestamptz not null default now()
);
create table if not exists reps.rate_limits (
 key text primary key, bucket bigint not null, count integer not null
);
create table if not exists reps.datasets (
 id text primary key, synced_at timestamptz
);
create table if not exists reps.tokens (
 hash text primary key, dataset text references reps.datasets not null,
 scope text not null check(scope in ('read','upload')), created_at timestamptz not null default now()
);
create table if not exists reps.workouts (
 sequence bigint generated always as identity primary key,
 dataset text references reps.datasets not null,
 id text not null, date text not null, record jsonb not null,
 unique(dataset,id)
);
create index if not exists workouts_date on reps.workouts(dataset,date,sequence);
alter table reps.activities enable row level security;
alter table reps.cheers enable row level security;
alter table reps.reports enable row level security;
alter table reps.subscribers enable row level security;
alter table reps.rate_limits enable row level security;
alter table reps.datasets enable row level security;
alter table reps.tokens enable row level security;
alter table reps.workouts enable row level security;
revoke all on all tables in schema reps from public;
alter table reps.datasets add column if not exists nickname text not null default 'RFP founder';
alter table reps.datasets add column if not exists autopost boolean not null default false;
-- Upload credential may sync all history; public posting is a separate operator opt-in.
alter table reps.datasets add column if not exists public_after timestamptz;
alter table reps.workouts add column if not exists posted boolean not null default false;
alter table reps.activities add column if not exists source_key text unique;
