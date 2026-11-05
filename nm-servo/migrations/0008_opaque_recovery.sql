CREATE TABLE opaque_recovery_credentials (
    user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    record bytea NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
