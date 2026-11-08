-- Banco dos logs de syslog (ADR 020), separado do principal: a ingestão e a
-- retenção dele não incham o banco do sistema nem o backup. O dono é o usuário
-- da imagem (`POSTGRES_USER`), que é quem roda este script. As tabelas nascem
-- pelas migrations do `LogsMigrator` no boot da API.
--
-- Só roda com o volume vazio (primeiro boot): é o contrato do
-- `/docker-entrypoint-initdb.d` da imagem oficial.
CREATE DATABASE netmonitor_logs;
