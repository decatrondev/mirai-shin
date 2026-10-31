# Wallpaper format (v1)

> In development: the app does not draw wallpapers yet. This format is what the library reads today.

A wallpaper is a **folder** inside the wallpapers folder, with a `manifest.json` and its files.

```
wallpapers/
  sunset/
    manifest.json
    media/loop.mp4
    preview.webp
```

The wallpapers folder is, by default, `~/.local/share/net.decatron.MiraiShin/wallpapers/` on Linux and `%APPDATA%\net.decatron.MiraiShin\wallpapers\` on Windows. It can be changed with `wallpapers_dir` in the settings.

## `manifest.json`

```json
{
  "schema_version": 1,
  "id": "sunset",
  "type": "video",
  "file": "media/loop.mp4",
  "title": "Sunset",
  "preview": "preview.webp",
  "author": "Ana",
  "source_url": "https://example.com/sunset",
  "tags": ["evening", "calm"]
}
```

| Field | Required | Notes |
|---|---|---|
| `schema_version` | yes | `1` |
| `id` | yes | Unique text. If two folders share an id, the first one (by folder name) wins and the other is marked `duplicate_id`. |
| `type` | yes | `image` or `video` |
| `file` | yes | Path relative to the folder. Images: `png`, `jpg`, `jpeg`, `webp`. Videos: `mp4`, `webm`, `mkv`. |
| `title` | yes | Text |
| `preview` | no | Image relative to the folder (`png`, `jpg`, `jpeg`, `webp`) |
| `author`, `source_url` | no | Text |
| `tags` | no | List of texts |

Rules:
- Paths must stay inside the folder: absolute paths, drive letters and `..` are rejected (`unsafe_path`). Symbolic links are not followed.
- Unknown fields are ignored, so a wallpaper made with a newer version still opens in an older one.
- A `schema_version` higher than 1 is read anyway and marked as made with a newer version.
- Folders whose name starts with `.` are skipped.

## Problems

An invalid wallpaper stays in the library with all its problems listed, never hidden. Each problem is a code, optionally followed by the field (`missing_field:title`):

| Code | Meaning |
|---|---|
| `missing_manifest` | The folder has no `manifest.json` |
| `unreadable_manifest` | `manifest.json` exists but could not be read |
| `invalid_json` | It is not valid JSON |
| `not_an_object` | The JSON is not an object `{ … }` |
| `missing_field` | A required field is missing |
| `wrong_type` | A field has the wrong type (for example a number instead of text) |
| `empty_field` | A text field is empty |
| `unsupported_schema` | `schema_version` is lower than 1 |
| `unknown_kind` | `type` is not `image` or `video` |
| `unsafe_path` | A path leaves the folder |
| `file_not_found` | The file does not exist in the folder |
| `extension_mismatch` | The extension does not match the type |
| `duplicate_id` | Another folder already uses this id |
