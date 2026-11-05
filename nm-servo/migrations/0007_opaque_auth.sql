CREATE TABLE opaque_server_setup (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    setup bytea NOT NULL
);
CREATE TABLE opaque_credentials (
    user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    record bytea NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
