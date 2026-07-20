CREATE TABLE IF NOT EXISTS instagram_oauth_states (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    creator_id UUID NOT NULL REFERENCES creators(id) ON DELETE CASCADE,
    state TEXT NOT NULL UNIQUE,
    return_path TEXT NOT NULL DEFAULT '/connections',
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS instagram_oauth_states_creator_idx
ON instagram_oauth_states(creator_id);

CREATE INDEX IF NOT EXISTS instagram_oauth_states_expires_idx
ON instagram_oauth_states(expires_at);
