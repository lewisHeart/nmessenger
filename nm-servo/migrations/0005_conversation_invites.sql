CREATE TABLE IF NOT EXISTS conversation_invites (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id uuid NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    inviter_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE(conversation_id, user_id)
);
CREATE INDEX IF NOT EXISTS conversation_invites_user ON conversation_invites(user_id);

