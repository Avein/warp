-- Wipe persisted window-tree state because `ProjectIdentity`'s serialized JSON
-- shape changed: `ProjectOrigin` collapsed from four variants
-- (`Config` / `Template` / `Default` / `Root`) to two
-- (`Config { config_name }` / `Template { template_name }`). Legacy rows that
-- tag a window as `Default` or `Root` would either fail to deserialize or
-- silently drop their identity; rather than ship a serde shim for a
-- personal-fork branch with no production data, we wipe and start clean.
-- The next launch auto-spawns a synthetic root tab.
--
-- Foreign keys are toggled off for the duration of the wipe so the cascading
-- referrers (`tabs`, `pane_nodes`, `pane_leaves`, `*_panes`, `panels`,
-- `app.active_window_id`, …) don't reject the `DELETE`. Any rows orphaned by
-- the wipe are inert — nothing reads them once their owning window is gone —
-- and the next `save_app_state` populates fresh rows with new auto-increment
-- IDs.

PRAGMA foreign_keys = off;
DELETE FROM windows;
UPDATE app SET active_window_id = NULL;
PRAGMA foreign_keys = on;
