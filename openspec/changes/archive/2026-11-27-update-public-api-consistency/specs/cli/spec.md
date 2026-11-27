## ADDED Requirements

### Requirement: Strict boolean flag parsing
Plain (non-valued) CLI flags such as `--json`, `--replace`, and `--key-stdin` MUST reject inline `=value` syntax with an error rather than silently treating `--flag=false` or any other value as true.

#### Scenario: Inline value on boolean flag rejected
- **WHEN** the user runs `tuse providers --json=false` or `tuse connect --replace=false ...`
- **THEN** the CLI exits with status 1 and prints an error stating that the flag does not take a value

### Requirement: Valued flag arguments with leading hyphen
Valued CLI flags passed as `--flag VALUE` SHALL accept values starting with `-` (such as `--key -secret`) unless the next token is `-h`, `--help`, or a known flag of that command, in which case parsing MUST fail with `{flag} needs a value`. Inline `--flag=VALUE` SHALL continue to accept any value.

#### Scenario: Hyphenated value accepted when not a known flag
- **WHEN** the user runs `tuse connect --base https://example.com --name gw --key -secret`
- **THEN** `-secret` is parsed as the value of `--key`

#### Scenario: Missing value before another known flag fails
- **WHEN** the user runs `tuse -c --resume abc`
- **THEN** the CLI exits with status 1 and prints `-c needs a value`

### Requirement: Help and version command validation
`tuse help` SHALL accept either no arguments (printing full usage, including `TUSE_GLOBAL_CONFIG` precedence) or a single known subcommand name (printing that subcommand's usage), and MUST reject unknown subcommands, unexpected flags, or extra arguments. `tuse version`, `-V`, and `--version` MUST reject extra flags or positional arguments.

#### Scenario: Subcommand help
- **WHEN** the user runs `tuse help connect`
- **THEN** the CLI exits with status 0 and prints the `connect` usage line

#### Scenario: Invalid help arguments rejected
- **WHEN** the user runs `tuse help --bogus` or `tuse help nope` or `tuse help connect extra`
- **THEN** the CLI exits with status 1 with an error message

#### Scenario: Version rejects extra flags
- **WHEN** the user runs `tuse version --version` or `tuse -V --version`
- **THEN** the CLI exits with status 1 with an unknown-flag error

### Requirement: Providers JSON matches config provider fields
`tuse providers --json` SHALL output saved providers as a JSON array of objects with only `name`, `type`, and `base` (matching `config.get.providers` field names) and MUST NOT expose keys, headers, or extra metadata.

#### Scenario: Saved providers JSON output
- **WHEN** a provider is saved with `tuse connect` and the user runs `tuse providers --json`
- **THEN** each array element contains only `name`, `type`, and `base`, and contains no API keys, headers, or `base_url`/`api` legacy keys
