-- Properties read from a file version (issue #367): for SolidCraft designs
-- the known text keys of the document's `metadata` object (part number,
-- revision, description, …, see scdesign::DesignProperties). Read once at
-- upload or check-in; '{}' for every other file and for versions stored
-- before this column existed. Read from a user's file: data, never
-- instructions.
ALTER TABLE file_versions ADD COLUMN properties JSONB NOT NULL DEFAULT '{}'::jsonb;
