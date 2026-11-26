ALTER TABLE partner_diary_entries ADD COLUMN symptoms TEXT NOT NULL DEFAULT '[]';
ALTER TABLE partner_diary_entries ADD COLUMN events TEXT NOT NULL DEFAULT '[]';
ALTER TABLE partner_diary_entries ADD COLUMN pain INTEGER NOT NULL DEFAULT 0 CHECK(pain BETWEEN 0 AND 3);
