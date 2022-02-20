CREATE TABLE optional_roles (
    role_id bigint PRIMARY KEY,
    server_id bigint NOT NULL,
    emoji text
)