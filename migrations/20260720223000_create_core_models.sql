CREATE EXTENSION IF NOT EXISTS pgcrypto;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'post_status') THEN
        CREATE TYPE post_status AS ENUM (
            'draft',
            'pending-review',
            'approved',
            'scheduled',
            'published',
            'failed',
            'rejected'
        );
    END IF;
END
$$;

CREATE TABLE IF NOT EXISTS creators (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    auth_subject TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL,
    display_name TEXT,
    avatar_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS instagram_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    creator_id UUID NOT NULL REFERENCES creators(id) ON DELETE CASCADE,
    instagram_user_id TEXT NOT NULL,
    username TEXT,
    access_token_ciphertext TEXT,
    refresh_token_ciphertext TEXT,
    token_expires_at TIMESTAMPTZ,
    connection_status TEXT NOT NULL DEFAULT 'connected',
    reconnect_reason TEXT,
    connected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    disconnected_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT instagram_accounts_status_check CHECK (
        connection_status IN ('connected', 'reconnect-needed', 'disconnected')
    ),
    CONSTRAINT instagram_accounts_creator_user_unique UNIQUE (creator_id, instagram_user_id)
);

CREATE TABLE IF NOT EXISTS media_assets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    creator_id UUID NOT NULL REFERENCES creators(id) ON DELETE CASCADE,
    storage_key TEXT NOT NULL,
    public_url TEXT,
    source TEXT NOT NULL,
    width INTEGER,
    height INTEGER,
    mime_type TEXT,
    license TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT media_assets_storage_key_unique UNIQUE (storage_key),
    CONSTRAINT media_assets_dimensions_check CHECK (
        (width IS NULL OR width > 0) AND (height IS NULL OR height > 0)
    )
);

CREATE TABLE IF NOT EXISTS generated_posts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    creator_id UUID NOT NULL REFERENCES creators(id) ON DELETE CASCADE,
    instagram_account_id UUID REFERENCES instagram_accounts(id) ON DELETE SET NULL,
    media_asset_id UUID REFERENCES media_assets(id) ON DELETE SET NULL,
    image_reference TEXT,
    header_text TEXT NOT NULL,
    paragraph_text TEXT NOT NULL,
    caption TEXT NOT NULL,
    status post_status NOT NULL DEFAULT 'draft',
    scheduled_at TIMESTAMPTZ,
    published_at TIMESTAMPTZ,
    failed_at TIMESTAMPTZ,
    failure_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT generated_posts_publish_state_check CHECK (
        (status = 'published' AND published_at IS NOT NULL)
        OR (status <> 'published')
    ),
    CONSTRAINT generated_posts_failure_state_check CHECK (
        (status = 'failed' AND failed_at IS NOT NULL)
        OR (status <> 'failed')
    )
);

CREATE TABLE IF NOT EXISTS posting_schedules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    creator_id UUID NOT NULL UNIQUE REFERENCES creators(id) ON DELETE CASCADE,
    timezone TEXT NOT NULL DEFAULT 'UTC',
    cadence TEXT NOT NULL,
    schedule_rule JSONB NOT NULL DEFAULT '{}'::JSONB,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    next_run_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS post_queue_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    creator_id UUID NOT NULL REFERENCES creators(id) ON DELETE CASCADE,
    post_id UUID NOT NULL UNIQUE REFERENCES generated_posts(id) ON DELETE CASCADE,
    scheduled_for TIMESTAMPTZ NOT NULL,
    queue_position INTEGER NOT NULL DEFAULT 0,
    locked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT post_queue_entries_position_check CHECK (queue_position >= 0),
    CONSTRAINT post_queue_entries_creator_slot_unique UNIQUE (
        creator_id,
        scheduled_for,
        queue_position
    )
);

CREATE INDEX IF NOT EXISTS creators_email_idx ON creators(email);
CREATE INDEX IF NOT EXISTS instagram_accounts_creator_idx ON instagram_accounts(creator_id);
CREATE INDEX IF NOT EXISTS media_assets_creator_idx ON media_assets(creator_id);
CREATE INDEX IF NOT EXISTS generated_posts_creator_status_idx ON generated_posts(creator_id, status);
CREATE INDEX IF NOT EXISTS generated_posts_scheduled_at_idx ON generated_posts(scheduled_at);
CREATE INDEX IF NOT EXISTS posting_schedules_next_run_idx ON posting_schedules(next_run_at)
WHERE is_active = TRUE;
CREATE INDEX IF NOT EXISTS post_queue_entries_creator_schedule_idx
ON post_queue_entries(creator_id, scheduled_for);

CREATE OR REPLACE FUNCTION set_updated_at_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS creators_set_updated_at ON creators;
CREATE TRIGGER creators_set_updated_at
BEFORE UPDATE ON creators
FOR EACH ROW
EXECUTE FUNCTION set_updated_at_timestamp();

DROP TRIGGER IF EXISTS instagram_accounts_set_updated_at ON instagram_accounts;
CREATE TRIGGER instagram_accounts_set_updated_at
BEFORE UPDATE ON instagram_accounts
FOR EACH ROW
EXECUTE FUNCTION set_updated_at_timestamp();

DROP TRIGGER IF EXISTS generated_posts_set_updated_at ON generated_posts;
CREATE TRIGGER generated_posts_set_updated_at
BEFORE UPDATE ON generated_posts
FOR EACH ROW
EXECUTE FUNCTION set_updated_at_timestamp();

DROP TRIGGER IF EXISTS posting_schedules_set_updated_at ON posting_schedules;
CREATE TRIGGER posting_schedules_set_updated_at
BEFORE UPDATE ON posting_schedules
FOR EACH ROW
EXECUTE FUNCTION set_updated_at_timestamp();

DROP TRIGGER IF EXISTS post_queue_entries_set_updated_at ON post_queue_entries;
CREATE TRIGGER post_queue_entries_set_updated_at
BEFORE UPDATE ON post_queue_entries
FOR EACH ROW
EXECUTE FUNCTION set_updated_at_timestamp();
