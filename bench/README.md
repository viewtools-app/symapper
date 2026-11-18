# Public recall benchmark

A reproducible measurement of how often `symapper` puts the files a real bug fix touched into its
map, compared with a plain `grep` baseline, on 10 well-known open-source repositories. Everything
here is mechanical: repos are pinned to commit SHAs and tasks are mined by a fixed rule, with no
hand picking.

## What is measured

For each task, the repo is checked out at the parent of a bug-fix commit. The query is the commit
subject (cleaned, see below), passed as `--focus` words. Truth is the set of source files that
the commit changed and that already existed at the parent (new files cannot be in a map of the old
tree). Recall is the share of truth files that appear in the map.

Modes:

| Mode | What it is |
|---|---|
| `sym1500` | `symapper <repo> --tokens 1500 --focus <word> ...` |
| `sym3000` | the same with `--tokens 3000` |
| `nofocus1500` | `symapper <repo> --tokens 1500`, no focus (structure ranking only) |
| `grep` | files ranked by how many focus words they contain (`git grep -i -w`, then match count), cut to the same K as `sym1500` (the number of source files in its map) |

Micro recall pools all truth files; macro recall averages per-task recall. Hit rank is the
position of a truth file in the map (or in the grep ranking), and the median is over all hits.

## Task selection rule

`bench/select-tasks.sh` walks first-parent history back from the pinned SHA and keeps, per repo,
the first 3 commits where:

- the subject contains the word `fix` (case-insensitive);
- the commit changes 1 to 6 source files (`rs go ts tsx js mjs py swift`, not `.d.ts`);
- at least one of them is a non-test file that exists at the parent commit.

Focus = subject with the conventional prefix (`fix:`, `fix(scope):`), `#123` references and
punctuation removed. The output is `bench/tasks.tsv` (30 tasks). Repos and SHAs are in
`bench/repos.tsv`.

Held-out set: `bench/tasks-held.tsv` (30 tasks) holds the next 3 matching commits per repo
(`PER_REPO=6`, minus the rows of `bench/tasks.tsv`). It was frozen before any 006 ranking change
and is never used for tuning. Run it with
`BENCH_REPOS=/tmp/pubbench/repos scripts/bench-recall.sh --set public bench/tasks-held.tsv /tmp/pubbench/out-held`.

Second held-out set: `bench/tasks-held2.tsv` (30 tasks) holds the next 3 matching commits per
repo (`PER_REPO=9`, minus the rows of `bench/tasks.tsv` and `bench/tasks-held.tsv`). It was
frozen before any 007 change; `bench/tasks-held.tsv` is a tuning set from 007 on.

## Rerun

```sh
bench/fetch-repos.sh /tmp/pubbench/repos
bench/select-tasks.sh /tmp/pubbench/repos > /tmp/tasks.tsv   # optional: must equal bench/tasks.tsv
BENCH_REPOS=/tmp/pubbench/repos scripts/bench-recall.sh --set public bench/tasks.tsv /tmp/pubbench/out
BENCH_REPOS=/tmp/pubbench/repos scripts/bench-recall.sh --set public bench/tasks-held2.tsv /tmp/pubbench/out-held2
```

Set `SYMAPPER_BIN` to the binary under test. Needs git, bash, awk and perl.

## Metrics

The summary adds `files_at1`, `files_at5`, `files_at10`, `files_at20` and
`tasks_at1`, `tasks_at5`, `tasks_at10`, `tasks_at20` for every mode. `files_atN` is the
share of all truth files found within the first N ranked paths. `tasks_atN` is the share
of tasks with at least one hit within those paths. Ratios use six decimals; a zero
denominator prints `-`. Per-task top-N follows from `hit_rank` in `results.csv`, which
contains all hit ranks of the task.

Reprint a saved run summary without repositories or a symapper binary:

```sh
bash scripts/bench-recall.sh --set public --summary /tmp/pubbench/out
```

## Results: 0.1.0 tune-set run

symapper 0.1.0 (released binary, macOS arm64), 30 tasks, 54 truth files, 10 repos. Median
wall time is for a warm cache; grep wall time was not measured.

| Mode | Micro recall | Macro recall | Median hit rank | Mean source files in map | Median wall (s) |
|---|---|---|---|---|---|
| sym1500 | 0.963 | 0.950 | 12 | 52.9 | 0.052 |
| sym3000 | 0.981 | 0.967 | 12 | 90.2 | 0.051 |
| nofocus1500 | 0.574 | 0.614 | 27 | 46.4 | 0.046 |
| grep, equal K | 0.889 | 0.882 | 3 | 52.9 | not measured |

Where grep wins: its median hit rank is better (3 against 12), and on `hono-dea44c8` grep found
both truth files while sym1500 found one. On recall, sym1500 finds 52 of 54 truth files and grep
finds 48 of 54. Per task, sym1500 reaches full recall on 28 of 30 tasks, grep on 25 of 30.
The two tasks symapper misses fully or partly: `fastapi-eb2f492` (0 of 1, grep also 0) and
`hono-dea44c8` (1 of 2).

## Task order (006)

The tables compare `sym1500` with grep at equal K (the number of source files in the
sym1500 map). Recall is in-map micro recall; files@N and tasks@N use ranked paths.

Tune set (`bench/tasks.tsv`): 30 tasks, 54 truth files.

| Build or mode | files@1 | files@5 | files@10 | files@20 | tasks@5 | Recall |
|---|---|---|---|---|---|---|
| 0.1.0 | 0.000 | 0.074 | 0.426 | 0.722 | 0.133 | 0.963 |
| 006 build (unreleased) | 0.222 | 0.611 | 0.667 | 0.778 | 0.767 | 0.963 |
| grep, equal K | 0.333 | 0.667 | 0.741 | 0.778 | 0.800 | 0.889 |

Held-out set (`bench/tasks-held.tsv`): 30 tasks, 63 truth files.

| Build or mode | files@1 | files@5 | files@10 | files@20 | tasks@5 | Recall |
|---|---|---|---|---|---|---|
| 0.1.0 | 0.000 | 0.048 | 0.365 | 0.556 | 0.067 | 0.873 |
| 006 build (unreleased) | 0.175 | 0.429 | 0.619 | 0.714 | 0.700 | 0.857 |
| grep, equal K | 0.222 | 0.476 | 0.635 | 0.746 | 0.833 | 0.794 |

On the held-out set, the 006 build (unreleased) puts 39 of 63 truth files in the first
10 paths against grep's 40, missing criterion A1. The share of tasks with a hit in the first
5 paths is 0.700 against grep's 0.833, meeting A2's threshold of 0.8 x grep. In-map recall is
0.857 against grep's 0.794, meeting A3. In-map recall fell from 0.873 (0.1.0) to 0.857.

On a private set of 10 held-out tasks from personal repos, recall fell from 0.712 (0.1.0)
to 0.652, missing A4's floor of 0.70. Grep's in-map recall was 0.697, below 0.1.0 but above
the 006 build. Files@10 rose from 0.197 to 0.318 (grep: 0.318).
Run time was unchanged: 0.99x of 0.1.0, measured warm as the median over the 10 public repos
(each repo's median of five interleaved runs).

The held-out set was frozen before any ranking change and run once on the final build.
Constants were chosen on the tune set only, using a 24-point grid. The subject "fix typing"
occurs in both public sets at different commits. These 30 + 30 tasks are a small sample.

## Recall floor (007)

With `--focus`, every file the 0.1.0 ranking prints at the same token budget stays in the map,
with signatures cut before files and a bare name line when needed; 006's task-first order stays,
and files@N of the 007 build equals the 006 build on every set, so only Recall differs.
Without focus, output is unchanged. The tables use `sym1500` and grep at equal K, as above.

Tune sets:

| Set | 0.1.0 | 006 build | 007 build (unreleased) | grep |
|---|---|---|---|---|
| `bench/tasks.tsv` | 0.963 | 0.963 | 0.981 | 0.889 |
| `bench/tasks-held.tsv` | 0.873 | 0.857 | 0.889 | 0.794 |

New held-out set (`bench/tasks-held2.tsv`): 30 tasks, 57 truth files.

| Build or mode | files@1 | files@5 | files@10 | files@20 | tasks@5 | Recall |
|---|---|---|---|---|---|---|
| 0.1.0 | 0.000 | 0.035 | 0.281 | 0.544 | 0.067 | 0.895 |
| 006 build (unreleased) | 0.140 | 0.439 | 0.596 | 0.754 | 0.633 | 0.912 |
| 007 build (unreleased) | 0.140 | 0.439 | 0.596 | 0.754 | 0.633 | 0.930 |
| grep, equal K | 0.175 | 0.474 | 0.596 | 0.737 | 0.667 | 0.842 |

The grep row uses the 006/007 K. At the 0.1.0 build's K, grep recall is 0.825.

No truth file found by 0.1.0 is lost: 0 lost on every tune and held-out set at 1500 and 3000
tokens. Recall is at least 0.1.0's and above grep's. On the new public held-out set,
files@10 is 0.596 against the required 0.439 (0.1.0 plus half of the 006 gain).

Private sets from personal repos:

| Set | 0.1.0 | 006 build | 007 build (unreleased) | grep |
|---|---|---|---|---|
| Tune, 10 tasks | 0.603 | 0.603 | 0.655 | 0.586 |
| Held, 10 tasks | 0.712 | 0.652 | 0.758 | 0.697 |
| New held-out, 8 tasks | 1.000 | 0.947 | 1.000 | 0.789 |

These are in-map recall values. The 006 private held-set recall drop below A4's 0.70 floor
is fixed: recall is now 0.758.

Median run time is 1.02x of 0.1.0 on the tune protocol and 1.03x on the held2 first tasks,
within 10% of 0.1.0. The per-repo worst is 1.13x (alamofire) on the tune protocol and
1.15x (alamofire) on held2. Both use the 006 protocol: warm, median over the 10 public
repos of each repo's median of five interleaved runs.

The public and private held2 sets were frozen before any 007 code change and run once on the
final build.

## Limitations

- A commit subject often names the changed code, which helps grep and symapper alike. Real task
  descriptions can be vaguer or longer.
- Recall of files is not task success. A file in the map still has to be read and understood.
- No LLM agent is in the loop. This measures the map, not an agent's result.
- 30 tasks is a small sample; several are docs or typo fixes (`fix typo`) that touch a source file.
  They were kept because the rule is mechanical.
- Ranks are not comparable across modes: a symapper rank counts map entries, a grep rank counts
  files in a different ordering. Since 006, the summary prints top-N metrics per mode.
- Pinned to one commit per repo, one machine, one binary version.
