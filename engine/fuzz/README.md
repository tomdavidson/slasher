fuzz-run.sh          fuzz-triage.sh         developer
    │                      │                     │
    ├─ runs fuzzer         │                     │
    ├─ crashes land in     │                     │
    │  artifacts/          │                     │
    │                      ├─ minimizes          │
    │                      ├─ seeds corpus       │
    │                      ├─ generates tests    │
    │                      │                     ├─ reviews tests
    │                      │                     ├─ fixes bugs
    │                      │                     ├─ commits everything
    │                      │                     │
    ├─ next run replays ◄──┘                     │
    │  corpus (including                         │
    │  old crashes)                              │
    └─ nextest catches ◄─────────────────────────┘
       regressions forever
