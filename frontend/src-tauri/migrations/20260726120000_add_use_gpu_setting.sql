-- Add GPU-toggle preference to the singleton settings row.
-- NULL = "default": resolved at read time to ON iff a GPU is detected.
-- Some(0) = user explicitly disabled GPU (force CPU inference).
-- Some(1) = user explicitly enabled GPU.
ALTER TABLE settings ADD COLUMN use_gpu INTEGER;
