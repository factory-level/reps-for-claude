-- The authenticated upload API may change only its dataset's posting preferences.
-- Token issuance and administration remain unavailable to the runtime role.
grant update(nickname,autopost,public_after) on reps.datasets to rfp_web;
