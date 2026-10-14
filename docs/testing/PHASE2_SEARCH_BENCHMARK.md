# Phase 2 Search & Retrieval Benchmark

## Purpose

This benchmark supplies repeatable evidence for the Phase 2 accessibility gate:

> Can an auditor, manager, or partner reach the correct document materially faster in Professional DocX than through their normal Windows File Explorer workflow?

It measures two different things:

1. **Search response time** — how quickly indexed results become usable after the user finishes a query.
2. **End-to-end retrieval time** — how long the user takes to reach and open the correct document.

The application displays search response time beside the result count. The benchmark sheet records the full retrieval task.

## Test Rules

Use the same Windows machine, user, file corpus, and network conditions for both tools in a comparison run.

Do not move or duplicate client files merely for this benchmark. Professional DocX should index the existing approved roots in place.

Record the environment before each benchmark session:

- Professional DocX commit/version.
- Windows version.
- CPU and memory.
- Local disk, server share, or NAS.
- Wired/Wi-Fi/VPN state where relevant.
- Number of indexed files.
- Approximate corpus size.
- Whether the run is **warm** or **cold**.

A **warm** run means the application/index and normal filesystem caches have already been used. A **cold** run starts after the application has been restarted and should be reported separately rather than mixed into warm medians.

## Required Retrieval Tasks

Use real representative documents, but anonymize client-sensitive names in any committed results.

| Task ID | Retrieval goal | Example Professional DocX query |
| --- | --- | --- |
| T01 | Find a specific income-tax-return document | `salamudd itr` |
| T02 | Find a specific month GST/RCM working | `rcm march` |
| T03 | Find the final trial balance | `final tb` or the firm's normal wording |
| T04 | Find a subcontractor ageing | `subcontractor ageing` |
| T05 | Find prior-year signed financial statements | `signed fs 2024` or the firm's normal wording |

The benchmark should use the user's natural query, not a specially optimized phrase learned only for the test.

## Professional DocX Procedure

For each task:

1. Start from the Home view with no query.
2. Start the task timer.
3. Press `Ctrl+K`.
4. Type the natural identifying fragment.
5. Use keyboard result navigation where practical.
6. Open the intended original.
7. Stop the task timer when the correct file opens.
8. Record elapsed time, keystrokes, clicks, wrong-file opens, and success/failure.
9. Record the search-response milliseconds shown beside the result count.

Search-as-you-type intermediate fragments are not recent-search history. Only a query that successfully opens a result is retained as a useful recent search.

## File Explorer Procedure

For the same task and corpus:

1. Start from the firm's normal File Explorer starting point.
2. Start the task timer.
3. Use the normal workflow the participant would actually use: folder traversal, Explorer search, or both.
4. Stop the task timer when the correct file opens.
5. Record the same metrics.

Do not force an artificially slow Explorer method. The comparison is against the user's genuine current workflow.

## Repetitions

For a useful small-team baseline:

- Run each task at least **5 times per tool**.
- Alternate the order of tools or randomize task order to reduce learning bias.
- Report the **median** end-to-end time for each task.
- Keep cold-start results separate.
- Note failures and wrong-file opens instead of discarding them.

## Phase 2 Evidence Targets

Initial engineering targets:

- Indexed normal-query response: **<300 ms where feasible**.
- Typical recently accessed item: **1–2 interactions**.
- Normal retrieval path: `Ctrl+K -> meaningful fragment -> Enter`.
- Professional DocX median end-to-end retrieval should be materially faster than the normal File Explorer workflow; a useful stretch target is roughly **3x faster** on the representative task set without increasing wrong-file openings.

The 3x figure is a usability target, not a guarantee. The measured data decides whether the Phase 2 accessibility gate is actually met.

## Recording Results

Copy `docs/testing/search-benchmark-template.csv` to a local working location before entering client-specific results. Do not commit client names, private paths, or confidential document titles to the repository.

Recommended local result location:

```text
output/benchmarks/search/
```

The repository already ignores `output/`.

## Review Questions

After each benchmark session, answer:

- Which queries exceeded 300 ms and why?
- Did exact/filename matches appear before fuzzy/path-only matches?
- Did any typo query fail to retrieve the intended file?
- How often did the participant open the wrong file?
- Which tasks still required folder knowledge?
- Did unavailable/missing linked files remain clearly distinguishable?
- Was File Explorer faster on any task? If so, preserve that case as a regression target.

Phase 2 should not be treated as complete merely because search works technically. The benchmark must show that retrieval is measurably better in realistic use.
