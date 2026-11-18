-- Default CAD program of a project group: the program TeamCraft Desktop
-- suggests for "Check out and open" (a format family, see
-- cadformat::families, e.g. 'inventor') and its version ('2026', optional).
-- '' = no default: the desktop takes the newest installed program that
-- fits the file.
ALTER TABLE projects
    ADD COLUMN default_cad TEXT NOT NULL DEFAULT '',
    ADD COLUMN default_cad_version TEXT NOT NULL DEFAULT ''
        CHECK (char_length(default_cad_version) <= 32);
