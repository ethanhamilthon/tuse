## 1. Matching
- [x] 1.1 Give token pickers a match key per item: the `value` for `/`, `#`, and args, the file name without `file_dir` for `@`.
- [x] 1.2 Match `/`, `#`, and arg pickers by case-insensitive prefix of the key; match `@` by case-insensitive substring with prefix matches first.
- [x] 1.3 Keep the whole-row search for `/history`, `/resume`, `/fork`, `/models`, `/tasks`.

## 2. Files
- [x] 2.1 In `files::list`, read the modified time (created time as fallback) and sort newest first before the cap.
- [x] 2.2 Keep `.git` hidden and the trailing `/` on folders.

## 3. Tests
- [x] 3.1 `#r` shows only `#release`, `/r` ignores descriptions, the query is case-insensitive.
- [x] 3.2 `@` puts prefix matches before substring matches, each group newest first.
- [x] 3.3 Row search in the modal pickers still matches the detail cells.

## 4. Docs
- [x] 4.1 Describe the matching and the `@` order in `docs/` where the pickers are mentioned.
