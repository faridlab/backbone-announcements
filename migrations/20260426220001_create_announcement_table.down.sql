-- Down: drop announcements.announcements table
DROP TABLE IF EXISTS announcements.announcements CASCADE;
DROP FUNCTION IF EXISTS announcements.announcements_audit_timestamp() CASCADE;
