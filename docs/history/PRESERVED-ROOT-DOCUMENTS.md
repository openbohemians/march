# Preserved March 4 documents

These documents were found as uncommitted files in the canonical March
working tree when the historical repositories were consolidated on
2026-09-23. Their contents describe the March 4 VM, its compile-time reference
graph, explicit-free semantics, specialization model, and the work summarized
at March 4 commit `8a7738b`.

The files were copied byte-for-byte and renamed only to place them beside the
rest of the March 4 documentation:

| Original working-tree path | Archived path | SHA-256 |
| --- | --- | --- |
| `SESSION_SUMMARY.md` | `docs/history/SESSION-SUMMARY-2024-12-05.md` | `ea141a33dbdc39ca2a24420deb7243b1d6bf7b82e704251fbf603be075daa5ed` |
| `doc/DESIGN.md` | `docs/design/MARCH-VM-DESIGN-v0.7.md` | `f772e3766897f3b154025b0094776d7c956b7b2606a291a14a9b4198b2e339a2` |
| `doc/DESIGNv4.md` | `docs/design/MARCH-VM-DESIGN-v0.4.md` | `ad28aae126145d7524096a4beeb97822e441ddee615269ff19333eab0b776bc9` |
| `doc/FORMAL.md` | `docs/design/FORMAL-MODEL.md` | `7a47da29b09286d19c7bf17f16e2ee3b7b01d08b56fe35afdefaee523fd32f5e` |

Their presence records historical design intent; it does not make every claim
in them correct. The later March 6 audit and experiments identify important
limits in the compile-time liveness argument.
