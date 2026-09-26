-- Down: drop announcements.announcement_reads table
DROP TABLE IF EXISTS announcements.announcement_reads CASCADE;
DROP FUNCTION IF EXISTS announcements.announcement_reads_audit_timestamp() CASCADE;
