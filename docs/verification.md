# Environment verification (spec section 13, items 1-4)

Measured 2026-11-06. Item 5 (detection quality): the Sonar duplication comparison is done, the internal repository check is pending (see below).

| # | Item | Result |
|---|------|--------|
| 1 | Opengrep musl binary on Alpine with `--network none` | Works. Opengrep 1.33.1, cosign `verify-blob` passed during image build. `opengrep scan --json-output=/tmp/o.json -f r.yaml .` exit=0, one finding. |
| 2 | Grammar crates with `tree-sitter` | All 7 grammars parse with `tree-sitter = 0.27.1` (locked): java 0.23, kotlin-ng 1.1, python 0.25, javascript 0.25, typescript 0.23 (TS and TSX), go 0.25. `bash scripts/cargo.sh test grammars` passes. No downgrade needed. |
| 3 | Gitea token scopes | Gitea 1.27.3 (image `gitea/gitea:1`). See below. |
| 4 | Gitea issue body limit | 2,000,000-byte body accepted (201). 60,000, 500,000 and 1,000,000 also 201. `MAX_STATE_BYTES` = 40,000 is safe. |
| - | Image size | `qualgate:dev` 148MB by `docker image ls` (target 150MB). Real filesystem is about 70MB; Docker Desktop's containerd store reports a larger number. Dropping `rules/tests` from the build context and stripping the release binary brought it from 153MB to 148MB. |

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

Measured end to end in Task 12 (`bash tests/gitea_it.sh`, real Gitea 1.27 container, real `qualgate publish`):

| Token scopes | Result |
|---|---|
| `write:issue,read:repository` | Everything passes: create issues and labels, rerun with no API writes, edit body, comment, close, reopen, leave `qualgate:ignored` issues alone. |
| `write:issue` only | `publish` fails with exit 2 and `Gitea 권한 오류 (403)`, because `publish` calls `GET /repos/{o}/{r}` to read the default branch and that needs `read:repository`. |

Required scopes for `qualgate publish`: `write:issue` and `read:repository`. The earlier statement that `write:issue` alone is enough for editing was untested and is replaced by the table above: editing works, but the tool as a whole needs `read:repository` too. The brief's `write:issue,write:repository,read:user` set is more than needed.

## Deviations from the brief

- Creating the fixture repo with the brief's token (`write:issue,write:repository,read:user`) failed: `POST /user/repos` requires `write:user`. Repo was created with basic auth (`qg` throwaway account) instead. Not a product requirement.
- Replaced `sleep 15` with a `/api/healthz` poll. Gitea container bound to `127.0.0.1:3999` and removed afterward.
- Extra body sizes (60KB, 500KB, 1MB) and an additional `write:issue,read:repository` token were probed.

## Item 5: detection quality (Task 12)

### Internal repository check

Status: pending: needs an internal repository from the user. No repository path was provided, so no rule hit counts, false positive ratio (20 samples), timing or unanalyzed file count exist yet. Command to run, `scan` only, never `publish`:

```bash
git clone <internal repo url> <clone path>
MSYS_NO_PATHCONV=1 docker run --rm --network none -v "<clone path>:/src:ro" -v "$PWD/.qualgate-out:/out" qualgate:dev scan
```

Then record per rule hit counts from `.qualgate-out/result.json`, the false positive ratio of 20 sampled findings, elapsed time, and `unanalyzed` count here. A rule above 30% false positives must be fixed or removed from the default pack.

### E2E samples (`bash tests/e2e/run.sh`, `--network none`)

One violation file, two clone files and one coverage report per language. All six pass: findings present, clone group present, coverage measured, no unanalyzed files.

| Lang | Findings | Clone groups | Coverage | Duplication | Scan time (incl. container start) |
|---|---|---|---|---|---|
| java | 1 (`java.system-out`) | 1 | 50.0 | 85.1 | 11s |
| kotlin | 3 | 1 | 50.0 | 82.4 | 11s |
| python | 1 | 1 | 50.0 | 96.7 | 10s |
| react | 1 | 1 | 50.0 | 91.9 | 8s |
| vue | 1 | 1 | 50.0 | 73.9 | 29s (cargo test ran concurrently) |
| go | 1 | 1 | 75.0 | 80.6 | 11s |

### Duplication versus Sonar Community

Sonar Community 26.11.0.134175 (loaded image, unmodified, `SONAR_ES_BOOTSTRAP_CHECKS_DISABLE=true` on Docker Desktop), scanner `sonarsource/sonar-scanner-cli` 8.3.1, project = `tests/e2e/repos/java` with `-Dsonar.sources=src`. Only the measures API was read. Container removed afterwards, image kept.

| | Duplication | Duplicated lines / total lines |
|---|---|---|
| Sonar `duplicated_lines_density` | 77.8% | 70 / 90 (2 blocks, 2 files) |
| qualgate `metrics.duplication` | 85.1% | 74 / 87 (Clone1.java 4-40, Clone2.java 4-40) |

Difference 7.3 percentage points, under the 10 point threshold, so no deeper investigation. Both tools found exactly the two clone files and nothing else. Sonar's duplicated range covers 35 lines per file and counts 90 total lines, qualgate's covers 37 of 87 (`text.lines().count()`). The two-line difference per file is probably how the closing-brace lines of the last block and the line total are counted; not confirmed.

## Final review fixes (measured 2026-11-06, Opengrep 1.33.1)

### Opengrep target filtering

With `--no-git-ignore --x-ignore-semgrepignore-files --max-target-bytes=0` and no `--exclude`, `paths.scanned` included a 1.5MB file, `.gitignore`d and `.semgrepignore`d files, `tests/`, `test/`, `src/test/`, `com/acme/test/`, `node_modules/`, `build/`, `dist/`, `vendor/` and a path with space, `#` and Hangul. Exit 0, `skipped_rules: []`, `errors: []` on the default pack.

Extensions in `paths.scanned` with a rule for `[javascript, typescript]` and no path filter: `js`, `jsx`, `mjs`, `cjs`, `ts`, `tsx`, `d.ts`, `min.js`. Never scanned: `mts`, `cts`. With the default pack (React rules only include `*.jsx`, `*.tsx`) plain `js`/`ts`/`mjs`/`cjs` are not scanned either. `kts` is scanned. Decision: keep these extensions in `Lang::of` (duplication and coverage still measure them) and do not require them in `paths.scanned`, so they never become permanent unanalyzed noise.

`--exclude` semantics differ from qualgate's globs: `--exclude=h.py` also drops `x/h.py`, `--exclude=docs/*.py` also drops `x/docs/f.py`, `--exclude='*.py'` drops everything. Only `**/<name>/**` matched qualgate's meaning, so only that form is passed (for speed) and qualgate filters the rest. Speed on 11,117 files (10,000 under `node_modules`/`vendor`): 8s with the two directory excludes, 27s without.

Engine errors: an invalid pattern gives exit 2 with pathless `level: error`, `rule_id` set (already fatal); an unknown language gives exit 8. A syntax error in a target gives `level: warn` with a path (unanalyzed, as before).

### Gitea markdown (1.27, `/markdown` API and notifications)

`@user` in a table cell links and notifies. `\@user` and `&#64;user` still render as mention links. `@` followed by U+200B does not link and sent no notification. `\[y](url)` inside a table cell still produced a link. Entities `&#91;`, `&lt;`, `&#96;` render literally. A mention inside the HTML state comment did not notify.
