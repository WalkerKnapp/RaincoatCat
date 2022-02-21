ALTER TABLE servers ADD COLUMN verified_role_id bigint;
ALTER TABLE servers ADD COLUMN verification_message_id bigint;
ALTER TABLE servers ADD COLUMN verification_emoji text;
ALTER TABLE servers ADD COLUMN verification_timeout bigint;
