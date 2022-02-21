CREATE TYPE punishment_type AS ENUM ('dunce', 'ban');

CREATE TABLE punishments (
    id bigserial PRIMARY KEY,
    user_id bigint NOT NULL,
    server_id bigint NOT NULL,
    punishment_type punishment_type NOT NULL,
    expires timestamp
);

CREATE TABLE punishment_removed_roles (
    id bigserial PRIMARY KEY,
    punishment_id bigint NOT NULL,
    role_id bigint NOT NULL
);

ALTER TABLE servers ADD COLUMN dunce_role_id bigint;
