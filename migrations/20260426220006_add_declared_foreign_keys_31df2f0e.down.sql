-- Down: drop the declared foreign keys for announcements module
ALTER TABLE IF EXISTS announcements.announcement_reads DROP CONSTRAINT IF EXISTS fk_announcement_reads_announcement_id;
