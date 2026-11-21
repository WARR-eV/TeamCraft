-- The duplicate guard of a chunked upload (issue #359): POST /api/v1/uploads
-- takes `refuse_clash` and `refuse_same_name`, and the session remembers them
-- until the upload is stored (finish, or the `uploads` field of a multipart
-- upload or check-in). With `refuse_clash` a new file of the revision is
-- refused when another item of the project group already has a current file
-- with the same folder and name; with `refuse_same_name` also when one has
-- the same name in another folder (services::file_check). FALSE = no guard,
-- what every existing session and every client that does not ask gets.
ALTER TABLE upload_sessions ADD COLUMN refuse_clash BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE upload_sessions ADD COLUMN refuse_same_name BOOLEAN NOT NULL DEFAULT FALSE;
