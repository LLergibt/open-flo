CREATE TABLE demo_messages (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    message TEXT NOT NULL CHECK (length(message) > 0)
);

INSERT INTO demo_messages (id, message)
VALUES (1, 'Привет из SQLite');
