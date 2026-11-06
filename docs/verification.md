# Environment verification (spec section 13, items 1-4)

Measured 2026-11-06. Item 5 (detection quality) is done in Task 12.

| # | Item | Result |
|---|------|--------|
| 1 | Opengrep musl binary on Alpine with `--network none` | Works. Opengrep 1.33.1, cosign `verify-blob` passed during image build. `opengrep scan --json-output=/tmp/o.json -f r.yaml .` exit=0, one finding. |
| 2 | Grammar crates with `tree-sitter` | All 7 grammars parse with `tree-sitter = 0.27.1` (locked): java 0.23, kotlin-ng 1.1, python 0.25, javascript 0.25, typescript 0.23 (TS and TSX), go 0.25. `bash scripts/cargo.sh test grammars` passes. No downgrade needed. |
| 3 | Gitea token scopes | Gitea 1.27.3 (image `gitea/gitea:1`). See below. |
| 4 | Gitea issue body limit | 2,000,000-byte body accepted (201). 60,000, 500,000 and 1,000,000 also 201. `MAX_STATE_BYTES` = 40,000 is safe. |
| - | Image size | `qualgate:dev` 132MB (target 150MB). |

## Opengrep JSON

`--json-output=FILE` works. Top-level keys: `version`, `results`, `errors`, `paths`, `interfile_languages_used`, `skipped_rules`.
`results[i]`: `check_id`, `path`, `start{line,col,offset}`, `end{line,col,offset}`, `extra{metavars,message,metadata,severity,fingerprint,lines,is_ignored,validation_state,engine_kind}`.
`extra.metadata` keeps custom keys verbatim: `qualgate_id == "t.x"`, `qualgate_severity`, `qualgate_category`.
`paths` = `{"scanned": [...]}`, `errors` = `[]` when clean. `check_id` is the bare rule id (`t`). Stdout also prints a human-readable report even with `--json-output`.

## Gitea token scopes

| Token scopes | Create issue | Create label | Read repo (`GET /repos/o/r`) | List issues |
|---|---|---|---|---|
| `write:issue` only | 201 | 201 | 403 (requires `read:repository`) | 200 |
| `write:issue,read:repository` | 201 | 201 | 200 | 200 |

Issue PATCH (edit body) with `write:issue,read:repository`: 201 (Gitea 1.27 returns 201 on PATCH).

Required scopes: `write:issue` is enough for create issue, create label, list/edit issues. Add `read:repository` only if the tool calls `GET /repos/{o}/{r}` itself. The brief's `write:issue,write:repository,read:user` set is more than needed.

## Deviations from the brief

- Creating the fixture repo with the brief's token (`write:issue,write:repository,read:user`) failed: `POST /user/repos` requires `write:user`. Repo was created with basic auth (`qg` throwaway account) instead. Not a product requirement.
- Replaced `sleep 15` with a `/api/healthz` poll. Gitea container bound to `127.0.0.1:3999` and removed afterward.
- Extra body sizes (60KB, 500KB, 1MB) and an additional `write:issue,read:repository` token were probed.
