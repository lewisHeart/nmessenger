CREATE TABLE users (
    id uuid PRIMARY KEY,
    email text NOT NULL UNIQUE,
    password_hash text NOT NULL,
    display_name text NOT NULL,
    personal_message text NOT NULL DEFAULT '',
    bio text NOT NULL DEFAULT '',
    presence text NOT NULL DEFAULT 'available'
        CHECK (presence IN ('available','busy','away','invisible','offline')),
    avatar_id uuid,
    scene text NOT NULL DEFAULT '',
    now_playing text,
    settings jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    disabled_at timestamptz,
    deleted_at timestamptz
);

CREATE TABLE banned_emails (
    email text PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE TABLE auth_sessions (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash text NOT NULL UNIQUE,
    device_name text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    last_seen_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz
);
CREATE INDEX sessions_user ON auth_sessions(user_id, expires_at);

CREATE TABLE encryption_keys (
    device_id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    public_key text NOT NULL CHECK (length(public_key) = 44),
    signing_key text NOT NULL CHECK (length(signing_key) = 44),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX encryption_keys_user ON encryption_keys(user_id);

CREATE TABLE encryption_key_sessions (
    device_id uuid NOT NULL REFERENCES encryption_keys(device_id) ON DELETE CASCADE,
    session_id uuid NOT NULL REFERENCES auth_sessions(id) ON DELETE CASCADE,
    PRIMARY KEY (device_id, session_id),
    UNIQUE (session_id)
);

CREATE TABLE contacts (
    owner_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    nickname text,
    favorite boolean NOT NULL DEFAULT false,
    blocked boolean NOT NULL DEFAULT false,
    state text NOT NULL CHECK (state IN ('pending_incoming','pending_outgoing','accepted')),
    details jsonb NOT NULL DEFAULT '{}',
    PRIMARY KEY(owner_id, user_id),
    CHECK (owner_id <> user_id)
);
CREATE INDEX contacts_target ON contacts(user_id, owner_id);

CREATE TABLE user_blocks (
    owner_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY(owner_id,user_id),
    CHECK(owner_id<>user_id)
);
CREATE INDEX blocks_target ON user_blocks(user_id,owner_id);

CREATE TABLE categories (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name text NOT NULL,
    position integer NOT NULL DEFAULT 0,
    UNIQUE(owner_id, id)
);

CREATE TABLE contact_categories (
    owner_id uuid NOT NULL,
    user_id uuid NOT NULL,
    category_id uuid NOT NULL,
    PRIMARY KEY(owner_id, user_id, category_id),
    FOREIGN KEY(owner_id,user_id) REFERENCES contacts(owner_id,user_id) ON DELETE CASCADE,
    FOREIGN KEY(owner_id,category_id) REFERENCES categories(owner_id,id) ON DELETE CASCADE
);

CREATE TABLE conversations (
    id uuid PRIMARY KEY,
    kind text NOT NULL CHECK (kind IN ('direct','group')),
    title text,
    owner_id uuid NOT NULL REFERENCES users(id),
    direct_key text UNIQUE,
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CHECK ((kind = 'direct') = (direct_key IS NOT NULL))
);

CREATE TABLE conversation_members (
    conversation_id uuid NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    joined_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    hidden_at timestamptz NOT NULL DEFAULT '-infinity',
    read_at timestamptz NOT NULL DEFAULT '-infinity',
    read_id uuid NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000',
    PRIMARY KEY(conversation_id,user_id)
);
CREATE INDEX member_user ON conversation_members(user_id,conversation_id);

CREATE TABLE files (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    conversation_id uuid REFERENCES conversations(id) ON DELETE CASCADE,
    filename text NOT NULL,
    mime text NOT NULL,
    size bigint NOT NULL CHECK (size BETWEEN 0 AND 26214400),
    sha256 text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX files_conversation ON files(conversation_id);
CREATE INDEX files_owner_size ON files(owner_id);

CREATE TABLE user_assets (
    id uuid PRIMARY KEY,
    owner_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind text NOT NULL CHECK (kind IN ('avatar','emoticon','background','sound')),
    name text NOT NULL,
    shortcut text,
    file_id uuid NOT NULL REFERENCES files(id),
    UNIQUE(owner_id, id)
);
CREATE INDEX assets_file ON user_assets(file_id);
ALTER TABLE users ADD CONSTRAINT avatar_asset FOREIGN KEY(avatar_id)
    REFERENCES user_assets(id) ON DELETE SET NULL;

CREATE TABLE messages (
    id uuid PRIMARY KEY,
    conversation_id uuid NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    sender_id uuid NOT NULL REFERENCES users(id),
    client_id uuid NOT NULL,
    kind text NOT NULL CHECK (kind IN ('text','nudge','wink','ink','voice','file','photo','system','activity')),
    body text NOT NULL,
    style jsonb NOT NULL,
    attachment_id uuid REFERENCES files(id),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE(sender_id,client_id)
);
CREATE INDEX message_history ON messages(conversation_id,created_at DESC,id DESC);
CREATE INDEX message_search ON messages USING gin(to_tsvector('simple',body));

CREATE TABLE message_outbox (
    message_id uuid PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
    recipients uuid[] NOT NULL,
    event jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    delivered_at timestamptz,
    next_attempt_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    attempts integer NOT NULL DEFAULT 0
);
CREATE INDEX message_outbox_pending ON message_outbox(next_attempt_at)
    WHERE delivered_at IS NULL;

CREATE TABLE message_deletion_outbox (
    id uuid PRIMARY KEY,
    recipients uuid[] NOT NULL,
    event jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    delivered_at timestamptz,
    next_attempt_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    attempts integer NOT NULL DEFAULT 0
);
CREATE INDEX message_deletion_outbox_pending ON message_deletion_outbox(next_attempt_at)
    WHERE delivered_at IS NULL;

CREATE TABLE activities (
    id uuid PRIMARY KEY,
    conversation_id uuid NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    initiator_id uuid NOT NULL REFERENCES users(id),
    kind text NOT NULL CHECK (kind IN ('audio_call','video_call','game','remote_assistance')),
    status text NOT NULL DEFAULT 'offered' CHECK(status IN ('offered','accepted','declined','ended')),
    payload jsonb NOT NULL DEFAULT '{}',
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX activity_conversation ON activities(conversation_id,status);

CREATE TABLE activity_participants (
    activity_id uuid NOT NULL REFERENCES activities(id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status text NOT NULL CHECK(status IN ('offered','accepted','declined','ended')),
    PRIMARY KEY(activity_id,user_id)
);

CREATE TABLE announcement_admins (
    email text PRIMARY KEY CHECK (email = lower(trim(email)))
);
COMMENT ON TABLE announcement_admins IS 'Legacy email allowlist; no longer used for authorization';

CREATE TABLE announcements (
    id uuid PRIMARY KEY,
    title text NOT NULL CHECK (char_length(title) BETWEEN 1 AND 120),
    body text NOT NULL CHECK (char_length(body) <= 2000),
    link_url text,
    image_file_id uuid REFERENCES files(id) ON DELETE SET NULL,
    enabled boolean NOT NULL DEFAULT true,
    full_banner boolean NOT NULL DEFAULT false,
    created_by uuid REFERENCES users(id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX announcements_display ON announcements(enabled, updated_at DESC);
CREATE INDEX announcements_image ON announcements(image_file_id) WHERE image_file_id IS NOT NULL;

CREATE TABLE app_admins (
    user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    granted_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE TABLE admin_audit (
    id bigserial PRIMARY KEY,
    actor_id uuid REFERENCES users(id) ON DELETE SET NULL,
    actor_email text NOT NULL,
    action text NOT NULL,
    target_user uuid REFERENCES users(id) ON DELETE SET NULL,
    target_session uuid,
    details jsonb NOT NULL DEFAULT '{}',
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX admin_audit_created ON admin_audit(created_at DESC,id DESC);

CREATE TABLE bots (
    user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    owner_id uuid NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    username text NOT NULL UNIQUE,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CHECK (user_id <> owner_id)
);
CREATE INDEX bots_owner ON bots(owner_id);

CREATE TABLE badges (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    slug text NOT NULL UNIQUE,
    name text NOT NULL,
    description text NOT NULL DEFAULT '',
    icon text NOT NULL DEFAULT '🏆',
    points int NOT NULL DEFAULT 10,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE TABLE user_badges (
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    badge_id uuid NOT NULL REFERENCES badges(id) ON DELETE CASCADE,
    awarded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    awarded_by uuid REFERENCES users(id) ON DELETE SET NULL,
    PRIMARY KEY (user_id, badge_id)
);
CREATE INDEX user_badges_user_id ON user_badges(user_id);
CREATE INDEX user_badges_badge_id ON user_badges(badge_id);

CREATE TABLE first_account_bootstrap (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    claimed boolean NOT NULL
);
INSERT INTO first_account_bootstrap(singleton, claimed)
VALUES (true, EXISTS(SELECT 1 FROM users))
ON CONFLICT (singleton) DO NOTHING;

CREATE TABLE github_release_deliveries (
    payload_hash text PRIMARY KEY,
    received_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE github_releases (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    published_at timestamptz NOT NULL,
    payload jsonb NOT NULL
);

-- Core badge catalog
INSERT INTO badges (slug, name, description, icon, points) VALUES
    ('fundadores', 'Fundador', 'Fez parte dos primeiros passos do NMessenger.', 'badges/founder.svg', 100),
    ('betas', 'Pioneiro Beta', 'Ajudou a experimentar o NMessenger durante a fase beta.', 'badges/beta.svg', 50),
    ('bom-de-papo', 'Bom de Papo', 'Compartilhou mais de 10 mil mensagens com a comunidade.', 'badges/chat.svg', 30),
    ('popular', 'Amigo de Todos', 'Reuniu mais de 500 contatos no NMessenger.', 'badges/community.svg', 30),
    ('orkut-nostalgia', 'Nostalgia Orkut', 'Celebra as amizades e lembranças da época do Orkut.', 'badges/orkut-nostalgia.svg', 25),
    ('vip', 'Destaque da Comunidade', 'Contribuiu de forma especial para a comunidade NMessenger.', 'badges/vip.svg', 50),
    ('desenvolvedor', 'Desenvolvedor', 'Participou do desenvolvimento e evolução do NMessenger.', 'badges/criador.svg', 75),
    ('1-ano-de-conta', '1 ano de amizade', 'Há um ano fazendo parte do NMessenger.', 'badges/1year.svg', 50),
    ('2-anos-de-conta', '2 anos de amizade', 'Há dois anos compartilhando momentos no NMessenger.', 'badges/2year.svg', 100),
    ('3-anos-de-conta', '3 anos de amizade', 'Há três anos construindo histórias no NMessenger.', 'badges/3year.svg', 150),
    ('4-anos-de-conta', '4 anos de amizade', 'Há quatro anos fazendo parte da comunidade.', 'badges/4year.svg', 200),
    ('5-anos-de-conta', '5 anos de amizade', 'Cinco anos de histórias e amizades no NMessenger.', 'badges/5year.svg', 250)
ON CONFLICT (slug) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    icon = EXCLUDED.icon,
    points = EXCLUDED.points;
