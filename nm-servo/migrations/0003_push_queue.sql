CREATE TABLE push_queue (
    message_id uuid NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    session_id uuid NOT NULL REFERENCES auth_sessions(id) ON DELETE CASCADE,
    attempts integer NOT NULL DEFAULT 0,
    next_attempt_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (message_id, session_id)
);
CREATE INDEX push_queue_due ON push_queue(next_attempt_at);
