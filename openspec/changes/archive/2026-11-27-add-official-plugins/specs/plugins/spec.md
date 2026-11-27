## ADDED Requirements

### Requirement: Official plugin catalog and installer selection
The repository SHALL publish `login`, `themes`, `status`, and `rename` as independent installable directories under `plugins/`, each with a manifest. The download installer SHALL offer each missing official plugin individually in an interactive terminal after installing the binary. It MUST require affirmative consent per plugin, MUST NOT replace existing plugins, and MUST NOT install plugins when run without an interactive terminal. Cancelling or declining a plugin MUST NOT undo the binary installation.

#### Scenario: Select only one plugin
- **WHEN** a user runs the download installer interactively, chooses `themes`, and declines the other plugins
- **THEN** the installer installs `themes` using `tuse plugins add` and does not install the other plugins

#### Scenario: Install without a terminal
- **WHEN** an unattended job runs the download installer without a terminal
- **THEN** the installer installs the binary without prompting or installing plugins

#### Scenario: Keep an existing plugin
- **WHEN** an official plugin is already installed
- **THEN** the installer skips it without replacing its contents
