-- No location is collected until a profile explicitly supplies a public label.
alter table reps.datasets add column if not exists location text check(location is null or length(trim(location)) between 1 and 80);
alter table reps.activities add column if not exists location text check(location is null or length(trim(location)) between 1 and 80);
grant update(location) on reps.datasets,reps.activities to rfp_web;
