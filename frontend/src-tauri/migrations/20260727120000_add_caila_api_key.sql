-- Migration: Add Caila API Key to settings table
-- Adds support for the Caila (Just AI) LLM provider via its OpenAI adapter.
ALTER TABLE settings ADD COLUMN cailaApiKey TEXT;
