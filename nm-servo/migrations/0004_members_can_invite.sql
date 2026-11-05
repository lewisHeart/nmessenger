ALTER TABLE conversations ADD COLUMN IF NOT EXISTS members_can_invite boolean NOT NULL DEFAULT true;

