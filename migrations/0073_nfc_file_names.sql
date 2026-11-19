-- File and folder names in Unicode NFC (composed).
--
-- macOS hands out decomposed names (NFD: "a" + U+0308 for "ä"). A CAD folder
-- imported from a Mac was stored that way, the desktop app on Windows wrote
-- the files to disk under those names, and Inventor, SOLIDWORKS & co. – which
-- look for the composed name they wrote into the assembly – did not find the
-- parts: NTFS and ext4 compare names code unit by code unit. The server now
-- composes every name it receives (util::sanitize_filename,
-- domain::normalize_folder, the reference names of cadrefs); this migration
-- composes the names stored before. Vault objects are keyed by content and do
-- not change. Running it again changes nothing.
--
-- normalize() needs a UTF8 database (TeamCraft creates them so); on another
-- encoding there are no decomposed names to repair and nothing happens.
DO $$
BEGIN
    IF current_setting('server_encoding') <> 'UTF8' THEN
        RETURN;
    END IF;

    UPDATE files SET filename = normalize(filename, NFC) WHERE filename IS NOT NFC NORMALIZED;
    UPDATE files SET folder = normalize(folder, NFC) WHERE folder IS NOT NFC NORMALIZED;
    UPDATE file_versions SET export_name = normalize(export_name, NFC)
     WHERE export_name IS NOT NFC NORMALIZED;
    UPDATE upload_sessions SET filename = normalize(filename, NFC) WHERE filename IS NOT NFC NORMALIZED;
    UPDATE upload_sessions SET folder = normalize(folder, NFC) WHERE folder IS NOT NFC NORMALIZED;
    UPDATE staged_imports SET filename = normalize(filename, NFC) WHERE filename IS NOT NFC NORMALIZED;
    UPDATE change_request_attachments SET filename = normalize(filename, NFC)
     WHERE filename IS NOT NFC NORMALIZED;
    -- item names the folder import took from such file names (the search
    -- typed on Windows did not find them)
    UPDATE revisions SET name = normalize(name, NFC) WHERE name IS NOT NFC NORMALIZED;

    -- References are unique per file version: a version that lists a name in
    -- both forms keeps one row (the composed one, else the first).
    DELETE FROM file_references r
     USING file_references o
     WHERE o.file_version_id = r.file_version_id
       AND r.name IS NOT NFC NORMALIZED
       AND o.name <> r.name
       AND normalize(o.name, NFC) = normalize(r.name, NFC)
       AND (o.name IS NFC NORMALIZED OR o.name < r.name);
    UPDATE file_references SET name = normalize(name, NFC) WHERE name IS NOT NFC NORMALIZED;
END $$;
