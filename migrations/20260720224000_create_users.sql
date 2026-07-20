CREATE TABLE IF NOT EXISTS users (
    sub TEXT PRIMARY KEY,
    email TEXT NOT NULL,
    name TEXT,
    picture_url TEXT,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE creators
ADD COLUMN IF NOT EXISTS user_sub TEXT REFERENCES users(sub) ON DELETE CASCADE;

CREATE UNIQUE INDEX IF NOT EXISTS creators_user_sub_unique_idx
ON creators(user_sub)
WHERE user_sub IS NOT NULL;

CREATE INDEX IF NOT EXISTS users_email_idx ON users(email);
