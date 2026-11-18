CREATE TABLE cycle_settings (
    tracker_id TEXT PRIMARY KEY NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    cycle_length INTEGER CHECK(cycle_length IS NULL OR cycle_length BETWEEN 10 AND 90),
    period_length INTEGER NOT NULL DEFAULT 5 CHECK(period_length BETWEEN 1 AND 14),
    context TEXT NOT NULL DEFAULT 'regular' CHECK(context IN ('regular','irregular','pregnancy','postpartum','hormonal_contraception','paused')),
    length_method TEXT NOT NULL DEFAULT 'auto' CHECK(length_method IN ('auto','manual'))
);
CREATE TABLE cycle_periods (
    id TEXT PRIMARY KEY NOT NULL,
    tracker_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    start_date TEXT NOT NULL,
    end_date TEXT,
    CHECK(end_date IS NULL OR end_date >= start_date),
    UNIQUE(tracker_id,start_date)
);
CREATE TABLE diary_entries (
    tracker_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    date TEXT NOT NULL,
    symptoms TEXT NOT NULL DEFAULT '[]',
    events TEXT NOT NULL DEFAULT '[]',
    mood TEXT NOT NULL DEFAULT '',
    flow TEXT NOT NULL DEFAULT '',
    pain INTEGER NOT NULL DEFAULT 0 CHECK(pain BETWEEN 0 AND 3),
    notes TEXT NOT NULL DEFAULT '',
    PRIMARY KEY(tracker_id,date)
);
