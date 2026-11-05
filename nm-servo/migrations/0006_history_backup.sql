CREATE TABLE encrypted_history_backups (
    user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    revision bigint NOT NULL CHECK (revision > 0),
    ciphertext text NOT NULL CHECK (octet_length(ciphertext) <= 131072),
    updated_at timestamptz NOT NULL DEFAULT now()
);
