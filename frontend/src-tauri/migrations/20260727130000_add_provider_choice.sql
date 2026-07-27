-- Migration: Track the onboarding LLM-provider choice separately from the
-- active `provider` column (which stays NOT NULL for backward compat).
--
-- `onboarding_provider_choice` values:
--   "local"              — user picked the local built-in model in onboarding
--   "cloud:<provider>"   — user picked a cloud provider (caila/openai/claude/...)
--   "deferred"           — user skipped LLM selection in onboarding (lazy-gate)
--   NULL                 — user is still in onboarding or on a pre-migration install
ALTER TABLE settings ADD COLUMN onboarding_provider_choice TEXT;

-- ISO-8601 timestamp of when the choice was recorded (for analytics / reminders).
ALTER TABLE settings ADD COLUMN provider_chosen_at TEXT;
