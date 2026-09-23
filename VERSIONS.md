# Preserved March lineages

The canonical March repository contains the earlier implementations as
namespaced, disconnected Git histories.  They are archive branches rather than
subdirectories of the current implementation, so their original commits remain
intact without cluttering the canonical working tree.

The `preserved/wip-2026-09-23` heads record source and design work that was
present but uncommitted when the histories were consolidated.  These commits
are archival snapshots, not assertions that the work was complete or passing.

## March 1

No remote was configured in the recovered local repository.

| Ref | Commit | Note |
| --- | --- | --- |
| `archive/march1/main` | `491a9136cd09f80a7e406a04ae08cf9b9b6a21ad` | Recovered main head |
| `archive/march1/preserved/wip-2026-09-23` | `4cab60ec7777671c5054a7007f53386a5811e47f` | Uncommitted implementation and test experiments |
| `archive/march1/v0.1.0` | `76b3754402cfee981661406b8f019b90ec760ec5` | Namespaced original tag |
| `archive/march1/v0.2.0` | `e06f2994be653b57550f25a2310048139e9aaa49` | Namespaced original tag |

## March 2

No remote was configured in the recovered local repository.

| Ref | Commit | Note |
| --- | --- | --- |
| `archive/march2/main` | `3c618a5f45f49225517557093a5d0713c7ef4b00` | Recovered main head |
| `archive/march2/bootstrap-forth` | `96e3f24fb90341bc49575c312583ca07c836c8af` | Bootstrap experiment head |
| `archive/march2/preserved/wip-2026-09-23` | `a5260859f45833480dea6ad8fbe2a56ac09b0501` | Uncommitted design state |

## March 3

March 3 appears never to have become an implementation.  No Git metadata was
present; only two design documents were recovered.  They are preserved
byte-for-byte in the one-commit `archive/march3/snapshot` history:

| File | Observed modification time | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `DESIGN.md` | 2025-10-08 21:20:13 -0400 | 6,369 | `49994d871294cec3776543ca5a260db84b8414d9986cdc4501dcbaeffe7379a8` |
| `EXAMPLE.md` | 2025-10-08 21:43:05 -0400 | 2,479 | `e16f2778f4db65ba9bf7829e9671b499bfcac6e2539bbf438a0e00ecc0898cf8` |

The snapshot commit is `b70c146f16eb0313232fe865d2234fe81fe8279b`.
`EXAMPLE.md` calls itself “March 2”; that text is preserved as found rather
than silently corrected.

## March 4

Original remote: `git@github.com:trans/march4.git`.

| Ref | Commit | Note |
| --- | --- | --- |
| `archive/march4/main` | `8a7738b36ada04fac4c889804094347b311708a5` | Local main head |
| `archive/march4/vm-computed-goto` | `c7a2ba331fe408df7ab95c590be0522629108da2` | Computed-goto VM experiment |
| `archive/march4/remotes/origin/main` | `4ea4b5e8561451837841ce83ec52948b2334277a` | Last recovered origin/main |
| `archive/march4/preserved/docs-2026-09-23` | `c40f2ee1a69b7bf5e6a93c4a36f582e89f2103b0` | Recovered session summary and VM/formal design documents |

## March 5

Original remote: `git@github.com:trans/march5.git`.

| Ref | Commit | Note |
| --- | --- | --- |
| `archive/march5/main` | `e91c734b1aba70c02bdaff0a003b5d489ac8c409` | Local main head |
| `archive/march5/pre-guard` | `e208d09ca18655c0f57640cb67b45849509defd1` | Pre-guard design head |
| `archive/march5/preserved/wip-2026-09-23` | `aa89906d4dad76244a424f6f30309b78dbac38ab` | Uncommitted INet, surface, YAML, and catalog work |
| `archive/march5/remotes/origin/main` | `9ca5d8501fac3a4d14d02065e9f29d01d24b340b` | Last recovered origin/main |

## Current March

The implementation currently under `march6/` is the active successor and is
tracked on the canonical repository branch.  The directory name is temporary:
if the research gates succeed, its contents will be promoted to the repository
root and it will simply be March.  A future architectural break can preserve
this lineage under `archive/march6/*` without requiring a separate repository.

The dated March 4/March 5 implementation audit is retained at
`doc/AUDIT-2026-09-23.md`.  It records the evidence available before March 6
was selected as the active research line; its recommendation is historical
context rather than the current architecture decision.

## Verification and access

List all preserved refs:

```sh
git for-each-ref refs/heads/archive refs/tags/archive
```

Inspect a lineage without disturbing the current working tree:

```sh
git log archive/march5/main
git worktree add ../march5-archive archive/march5/main
```

The source working copies should not be removed until these refs have been
pushed to the canonical remote and verified from a fresh clone.
