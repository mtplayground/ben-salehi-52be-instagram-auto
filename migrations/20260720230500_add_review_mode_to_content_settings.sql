ALTER TABLE creator_content_settings
ADD COLUMN IF NOT EXISTS review_mode_enabled BOOLEAN NOT NULL DEFAULT TRUE;
