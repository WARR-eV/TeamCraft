-- Trigram indexes of the substring search, created strictly (issue #308).
--
-- 0023_performance_indexes.sql and 0031_attribute_search_index.sql wrapped
-- the extension *and* the indexes in `EXCEPTION WHEN OTHERS`: any failure,
-- not only a missing pg_trgm, left the indexes silently absent and the item
-- search slow. Published migrations stay unchanged, so this one repairs
-- databases where they failed and narrows what is tolerated:
--
-- * Only creating the extension may fail, and only because it is not
--   installed on the server (undefined_file, feature_not_supported) or the
--   role may not create it (insufficient_privilege). The search then stays
--   correct and reads the tables; the server warns at start-up
--   (db::check_search_indexes).
-- * With the extension present, every index is created if missing, and any
--   error stops the migration instead of being swallowed.
DO $$
BEGIN
    BEGIN
        CREATE EXTENSION IF NOT EXISTS pg_trgm;
    EXCEPTION
        WHEN undefined_file OR feature_not_supported OR insufficient_privilege THEN
            RAISE NOTICE 'pg_trgm not available (%): item search without trigram indexes', SQLERRM;
    END;
    IF EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'pg_trgm') THEN
        CREATE INDEX IF NOT EXISTS items_number_trgm_idx ON items USING gin (number gin_trgm_ops);
        CREATE INDEX IF NOT EXISTS revisions_name_trgm_idx ON revisions USING gin (name gin_trgm_ops);
        CREATE INDEX IF NOT EXISTS revisions_description_trgm_idx ON revisions USING gin (description gin_trgm_ops);
        CREATE INDEX IF NOT EXISTS revisions_material_trgm_idx ON revisions USING gin (material gin_trgm_ops);
        CREATE INDEX IF NOT EXISTS files_filename_trgm_idx ON files USING gin (filename gin_trgm_ops);
        CREATE INDEX IF NOT EXISTS revisions_attribute_values_trgm_idx ON revisions
            USING gin ((jsonb_path_query_array(attributes, '$.*')::text) gin_trgm_ops);
    END IF;
END $$;
