CREATE TABLE partner_diary_entries (
    pair_id TEXT NOT NULL REFERENCES pairs(id) ON DELETE CASCADE,
    date TEXT NOT NULL,
    mood TEXT NOT NULL DEFAULT '',
    notes TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (pair_id, date)
);
