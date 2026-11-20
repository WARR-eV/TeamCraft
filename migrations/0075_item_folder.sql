-- The folder of an item inside its project group: where the file browser
-- (GET /api/v1/projects/{code}/folders, the desktop Home *Files* view) lists
-- the item while its latest revision has no file, and where new files of the
-- item go when an upload names no folder. Set when the item is created (the
-- *New item* form, POST /api/v1/items `folder`, *New item here* in the
-- desktop app) and changed with PATCH /api/v1/items/{number} `folder`.
-- '' = the top of the group – what every item had before, so nothing
-- changes for existing items. The application normalises the value
-- (domain::normalize_folder), the CHECK repeats the rules of files.folder.
ALTER TABLE items ADD COLUMN folder TEXT NOT NULL DEFAULT ''
    CHECK (length(folder) <= 400
           AND folder NOT LIKE '/%'
           AND folder NOT LIKE '%/'
           AND position(E'\\' IN folder) = 0
           AND folder !~ '(^|/)\.\.?(/|$)');
