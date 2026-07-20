ALTER TABLE generated_posts
ADD COLUMN IF NOT EXISTS publish_retry_count INTEGER NOT NULL DEFAULT 0,
ADD COLUMN IF NOT EXISTS last_publish_attempt_at TIMESTAMPTZ,
ADD COLUMN IF NOT EXISTS next_retry_at TIMESTAMPTZ;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'generated_posts_publish_retry_count_check'
    ) THEN
        ALTER TABLE generated_posts
        ADD CONSTRAINT generated_posts_publish_retry_count_check CHECK (publish_retry_count >= 0);
    END IF;
END
$$;

CREATE INDEX IF NOT EXISTS generated_posts_next_retry_idx
ON generated_posts(next_retry_at)
WHERE next_retry_at IS NOT NULL;
