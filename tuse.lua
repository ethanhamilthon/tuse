return {
  openspec = require("./openspec"),
  tuse_repo = {
    prompts = {
      release = [[
Make a new release, new patch version.
Delete all trash from the repo, make a commit, tag and push new version.
Wait for CI, make sure they didnt fail and tell me to update Tuse.
]],
    },
    systems = {
      [[
## Tuse Project Rules
You are inside the repo of Tuse, the AI agent you are.
The project is written in Rust; write idiomatic, correct Rust code.

Code rules:
- Zero comments. Never write comments.
- Do not edit README unless I ask for it.
- Keep production Rust code under 20000 lines (excluding tests). If it goes over, warn and simplify the functionality.
- Minimal tests.
- Everything in English (UI, contracts, docs).
- Minimal temporary files inside the repository; it must stay clean.
]],
    },
  },
}
