-- Usage statistics (#383): the desktop apps report anonymous daily counts to
-- their own TeamCraft server; the server keeps them here and – only when an
-- administrator switches it on – sends an aggregate to the TeamCraft
-- developers. No user, item, file, group or server names are stored.

-- One report per desktop installation and day (the installation id is a
-- random UUID made on that computer, not derived from anything).
CREATE TABLE usage_reports (
    day            DATE        NOT NULL,
    install_id     UUID        NOT NULL,
    version        TEXT        NOT NULL CHECK (char_length(version) <= 32),
    os             TEXT        NOT NULL CHECK (os IN ('windows', 'macos', 'linux', 'other')),
    arch           TEXT        NOT NULL CHECK (arch IN ('x86_64', 'aarch64', 'other')),
    package        TEXT        NOT NULL CHECK (char_length(package) <= 32),
    active_minutes INTEGER     NOT NULL CHECK (active_minutes BETWEEN 0 AND 1440),
    actions        JSONB       NOT NULL DEFAULT '{}'::jsonb,
    cad_exports    JSONB       NOT NULL DEFAULT '{}'::jsonb,
    received_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (day, install_id)
);

CREATE INDEX usage_reports_received ON usage_reports (received_at);

-- The instance's own switch and state (one row): whether the aggregate is
-- shared with the developers, the random instance id and what was sent last.
CREATE TABLE usage_sharing (
    id          BOOLEAN     PRIMARY KEY DEFAULT true CHECK (id),
    share       BOOLEAN     NOT NULL DEFAULT false,
    instance_id UUID        NOT NULL,
    updated_by  UUID        REFERENCES users (id) ON DELETE SET NULL,
    updated_at  TIMESTAMPTZ,
    last_sent_at TIMESTAMPTZ,
    last_sent   JSONB
);
