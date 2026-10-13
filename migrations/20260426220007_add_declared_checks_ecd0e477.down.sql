-- Down: drop the declared rule constraints for announcements module
ALTER TABLE IF EXISTS announcements.announcement_reads DROP CONSTRAINT IF EXISTS ck_announcement_reads_user_id_required__required;
ALTER TABLE IF EXISTS announcements.announcement_reads DROP CONSTRAINT IF EXISTS ck_announcement_reads_announcement_id_required__required;
ALTER TABLE IF EXISTS announcements.announcements DROP CONSTRAINT IF EXISTS ck_announcements_audience_unit_id_required__required;
ALTER TABLE IF EXISTS announcements.announcements DROP CONSTRAINT IF EXISTS ck_announcements_body_max_length__max_length;
ALTER TABLE IF EXISTS announcements.announcements DROP CONSTRAINT IF EXISTS ck_announcements_body_required__required;
ALTER TABLE IF EXISTS announcements.announcements DROP CONSTRAINT IF EXISTS ck_announcements_title_max_length__max_length;
ALTER TABLE IF EXISTS announcements.announcements DROP CONSTRAINT IF EXISTS ck_announcements_title_required__required;
