## ADDED Requirements

### Requirement: Plugins CLI subcommand integration
The `tuse` CLI SHALL expose `tuse plugins` with subcommands `add`, `list`, `delete`, `print`, and `update [repo]`, include `tuse plugins` in top-level usage (`tuse --help`) and subcommand help (`tuse plugins -h`), and reject unknown `tuse plugins` subcommands or unexpected flags with exit status 1 and usage output.

#### Scenario: Plugins help and usage
- **WHEN** the user runs `tuse --help` or `tuse plugins --help`
- **THEN** the CLI exits with status 0 and lists `tuse plugins` and its subcommands `add`, `list`, `delete`, `print`, and `update`

#### Scenario: Unknown plugins subcommand rejected
- **WHEN** the user runs `tuse plugins bogus`
- **THEN** the CLI exits with status 1 and prints an error with `tuse plugins` usage

### Requirement: Init generates a namespaced starter config
`tuse init` SHALL create `./tuse.lua` with a valid namespaced starter config that relies on automatic `global` and `plugins` loading rather than manual `require("global")` mutation, and SHALL continue to fail if `./tuse.lua` already exists.

#### Scenario: Generated project config loads under namespaced schema
- **WHEN** the user runs `tuse init` in an empty folder and starts `tuse`
- **THEN** `./tuse.lua` is created and loads cleanly under the namespaced config schema without manually calling `require("global")`
