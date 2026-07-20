CREATE TABLE IF NOT EXISTS creator_content_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    creator_id UUID NOT NULL UNIQUE REFERENCES creators(id) ON DELETE CASCADE,
    theme_topic TEXT NOT NULL,
    style_notes TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT creator_content_settings_theme_topic_check CHECK (
        char_length(trim(theme_topic)) BETWEEN 3 AND 180
    ),
    CONSTRAINT creator_content_settings_style_notes_check CHECK (
        char_length(trim(style_notes)) BETWEEN 3 AND 1200
    )
);

CREATE INDEX IF NOT EXISTS creator_content_settings_creator_idx
ON creator_content_settings(creator_id);

DROP TRIGGER IF EXISTS creator_content_settings_set_updated_at ON creator_content_settings;
CREATE TRIGGER creator_content_settings_set_updated_at
BEFORE UPDATE ON creator_content_settings
FOR EACH ROW
EXECUTE FUNCTION set_updated_at_timestamp();
