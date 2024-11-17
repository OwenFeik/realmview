PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS users (
    uuid TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    salt TEXT NOT NULL, -- CHAR(64)
    hashed_password TEXT NOT NULL, -- CHAR(64)
    recovery_key TEXT NOT NULL -- CHAR(64)
) STRICT;

CREATE TABLE IF NOT EXISTS user_sessions (
    session_key TEXT PRIMARY KEY, -- CHAR(64)
    user TEXT REFERENCES users(uuid) ON DELETE CASCADE NOT NULL,
    start_time INTEGER NOT NULL,
    end_time INTEGER
) STRICT;

CREATE TABLE IF NOT EXISTS media (
    uuid TEXT PRIMARY KEY,
    user TEXT REFERENCES users(uuid) ON DELETE CASCADE NOT NULL,
    relative_path TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    hashed_value TEXT NOT NULL, -- CHAR(64)
    file_size INTEGER NOT NULL, -- Size in bytes
    w REAL NOT NULL, -- Default width of tokens created with this media
    h REAL NOT NULL, -- Default height of tokens created with this media
    UNIQUE(user, hashed_value)
) STRICT;

CREATE TABLE IF NOT EXISTS projects (
    uuid TEXT PRIMARY KEY,
    user TEXT REFERENCES users(uuid) ON DELETE CASCADE NOT NULL,
    updated_time INTEGER NOT NULL,
    title TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS scenes (
    uuid TEXT PRIMARY KEY NOT NULL,
    project TEXT REFERENCES projects(uuid) ON DELETE CASCADE NOT NULL,
    updated_time INTEGER NOT NULL,
    title TEXT NOT NULL,
    thumbnail TEXT -- Relative URL for thumbnail
) STRICT;
