# RFP activity site

Next.js on Vercel, with a private `reps` schema in Supabase Postgres. No public
accounts, authentication pages, or browser database credentials. All browser
mutations go through validated server endpoints. SQL is parameterized, tables
have RLS enabled, and the schema grants no access to public roles.

## Setup

1. Create a Supabase project and obtain its transaction-pooler connection string.
2. Set `DATABASE_URL` and a random `RATE_LIMIT_SECRET` in server-side environment
   variables. Use a different database and secret for previews. Never use
   `NEXT_PUBLIC_` for either value. Postgres.js uses `prepare: false` for the pooler.
3. Run `pnpm operator migrate` with that database environment. It creates only
   the `reps` schema and its objects. Use an owner connection for migrations only. Apply the runtime-role migration
   and provision its password separately; the deployed server uses `rfp_web`.
4. Deploy this directory as a Next.js project on Vercel. Public posts and lead
   signup remain disabled when the database is unconfigured.

`pnpm dev` starts the local site. `pnpm build` checks and builds production.
Environment files are ignored; the operator command uses the process environment
(use Node's `--env-file` or your secret manager when running it).

## Operator commands

```sh
pnpm operator dataset                        # prints private dataset ID
pnpm operator token DATASET upload           # prints credential once + hash ID
pnpm operator token DATASET read
pnpm operator autopost DATASET 'RFP founder' # opt-in from now forward
pnpm operator stop-posting DATASET
pnpm operator revoke TOKEN_HASH
pnpm operator reports
pnpm operator feature POST_ID
pnpm operator unfeature POST_ID
pnpm operator remove POST_ID
pnpm operator subscribers > subscribers.csv
pnpm operator unsubscribe person@example.com
```

Store issued credentials directly in protected files, not shell arguments or
logs. `autopost` sets a persistent public opt-in timestamp, preventing historical
workout backfill from becoming public. Stopping public posting preserves queued
records; reenabling resumes them. Automatic public records retain a unique
source key even under concurrent upload retries. Deleting a post does not
recreate it during later syncs.

Public users receive a deletion token, stored in their browser and displayed as
a private deletion link. If lost, they can report a post for operator removal.
There are no photos, comments, rankings, or fabricated starter posts. Featured
people are the nicknames attached to selected real activity cards.

## Interfaces

- `GET/POST /api/activity`; GET accepts cursor and limit.
- `DELETE /api/activity/:id` with `{token}`.
- `POST /api/activity/:id/cheer` and `/report`.
- `POST /api/subscribe` with `{email}`. This captures leads, not email delivery.
- `GET /api/v1/history|summary|status` requires a read-only bearer token.
- `POST /api/v1/sync` requires an upload bearer token, accepts up to 100 records,
  returns accepted stable IDs, and drains at most six public posts per hour.

Private JSON uses `schemaVersion: 1`, `source`, `syncedAt`; history adds `records`
and `nextCursor`. Date filters are inclusive ISO calendar dates. Record fields
match the Rust `HistoryRecord` type. Public and private cursors are independent.

## Tests

```sh
pnpm test
TEST_DATABASE_URL=postgresql://.../DISPOSABLE_DB pnpm test
TEST_DATABASE_URL=postgresql://.../DISPOSABLE_DB pnpm exec tsx scripts/verify-cli.ts
```

Integration tests truncate the `reps` schema's test tables. Use only a disposable
database. The CLI test expects a local site on port 3017 using that same database
and a built `../app/src-tauri/target/release/reps`. It creates and removes its own
isolated dataset and never touches normal desktop history.

## Free-plan usage guards

The owner confirmed Supabase Free and Vercel Hobby on 2026-09-25. Never upgrade
plans or enable paid add-ons automatically. Free quotas are shared with other
projects, and exceeding them may interrupt service.

- Desktop sync: every five minutes (about 8,640 attempts per 30 days
  for one continuously running process; manual syncs/restarts add requests), with failure backoff up to one hour.
- All API routes share atomic database budgets: 2,000 requests per UTC day and
  40,000 per fixed, epoch-aligned 30-day window. Rejected calls still reach
  Vercel and execute the budget check; this is not a provider billing cap.
- Public feed responses can be cached at the CDN for 60 seconds. Private
  responses and failures are never cached. Different query strings have separate
  cache entries; caching is an optimization, not an abuse guarantee.
- Public posts: 10/hour per hashed IP bucket; auto-posts: 6/hour per dataset.
  Email signup: 5/hour per bucket. Only 256 rate buckets per public action
  prevent unbounded counter rows; visitors can share a bucket.
- Cloud storage pauses at 20,000 activities, 20,000 workout records, 5,000 email
  subscribers, 50,000 cheers, or 10,000 reports. Serialized insert triggers enforce
  caps across concurrent connections and allow idempotent retries at capacity.
- Full cloud storage returns 429 with retry guidance. Local SQLite history is
  retained; uploads stay queued and the desktop backs off. No automatic history
  deletion and no paid email sending, realtime subscriptions, media storage, or AI calls.

These are conservative application guards, not guarantees of provider quota
availability: rejected traffic, builds, database overhead, and other projects
also consume resources. The providers' free plans prevent usage overage billing.

## Desktop profiles and local endpoints

`GET/POST /api/v1/profile` requires the dataset's upload token. POST accepts `nickname` (1–32 characters) and/or `sharing` (boolean). It cannot select a different dataset. Enabling sharing starts a new public boundary; workouts recorded while sharing was off remain private. Nickname-only changes retain the existing boundary and sharing setting. Apply `supabase/migrations/20260925215359_rfp_profile_settings.sql` after the runtime-role migration.

The desktop's `rfp site --url URL --upload-token-file FILE --read-token-file FILE` selects a site and turns on sharing for a newly configured destination. `rfp profile` inspects or updates the site's profile. The daemon uses that endpoint automatically. Use an isolated database for local development; issue tokens for a local dataset with the operator commands above. Store only the raw `token` field in each mode-0600 token file. Once both destinations are configured, switch back using only `rfp site --url https://reps-for-prompts.vercel.app`.

Run `TEST_DATABASE_URL=... pnpm exec tsx scripts/verify-profile-cli.ts` after the API tests and Rust release build to verify two real HTTP destinations, profile names, default sharing, independent acknowledgments, and retry deduplication. Tests use disposable dataset credentials and never send requests to the production site.

## Guest consistency showcase

The homepage is a viewport-sized two-column one-pager: an explanation and GitHub link on the left, guest routine heatmaps on the right. No public workout-entry form or ranking. Anonymous `POST /api/activity` returns 405; authenticated sync creates posts. Legacy post deletion remains supported.

`GET /api/consistency` returns at most 24 sharing-enabled guests, their optional location, 28 date cells, latest routine percentage and a percentage-point delta comparing tracked days in the previous two completed seven-day windows. It never treats missing days as failures or invents past routine targets. Guests appear by recent sync, not score. Location defaults to null; `rfp profile --location "City, Region"` opts in, and `--clear-location` clears both the profile and past post labels transactionally. The feed and showcase are no-store so cleared location cannot persist in shared CDN caches.

Sync accepts optional `routineDays` (up to 28 date/completed/target/updatedAt snapshots). The desktop records targets as the routine runs, including later target changes; server merges retain the latest snapshot per date, bounded to 28 entries per dataset. Routine snapshots are private when public sharing is off. No new accounts or verification machinery.

The page refreshes every 90 seconds only while visible and online; failures back off and 429 waits an hour. Auto-scroll pauses on hover, focus, user pause, hidden page, and reduced-motion preference. If fewer than four real guests are present, deterministic demo profiles fill the remaining slots. Every demo is labeled and exists only in the browser; none enters the database, feed, or real metrics.

Apply `20260925222120_rfp_opt_in_location.sql` and `20260925222819_rfp_routine_progress.sql` before deploying this version. They add nullable location labels and bounded routine snapshots, with column-scoped runtime grants only.
