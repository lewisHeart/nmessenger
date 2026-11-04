CREATE TABLE push_devices (
    session_id uuid PRIMARY KEY REFERENCES auth_sessions(id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    endpoint text NOT NULL,
    public_key text NOT NULL,
    auth_secret text NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now()
);
