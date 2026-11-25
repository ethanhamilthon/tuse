## ADDED Requirements
### Requirement: Command descriptions and words
The `/` picker SHALL show the `desc` of a custom command as its detail. Picking a command with `args` SHALL put `/name ` in the input and open a picker of its `args`; a space typed after `/name` SHALL reopen that picker, filtered by the typed text. Choosing an arg SHALL clear the input and run the command with it. Esc in that picker SHALL leave `/name ` in the input, and Enter then SHALL run the command with no arg. Enter on `/name` followed by words SHALL run the command with those words as the config spec describes, and a word list the command does not accept MUST NOT run it and MUST NOT be sent as a message. Enter on the exact `/name`, or picking a command without `args` from the `/` picker, SHALL run it with no words. JSON-RPC `commands.run` SHALL accept `{name, argv?}` with a list of words, apply the same checks, and fail with `-32000` when they fail; `commands.list` SHALL return `[{name, desc, args, params}]`.

#### Scenario: Pick an arg
- **WHEN** the user picks `/deploy` and then `prod`
- **THEN** `run("prod")` is called and the input is cleared

#### Scenario: Esc then Enter
- **WHEN** the user picks `/deploy`, presses Esc in the arg picker, and presses Enter
- **THEN** `run(nil)` is called

#### Scenario: Type words
- **WHEN** the user types `/mv a.txt b.txt` and presses Enter
- **THEN** `run("a.txt", "b.txt")` is called and the text is not sent to the agent

#### Scenario: Rejected words
- **WHEN** the user sends `/deploy nope`
- **THEN** nothing runs and an error naming the command is shown

#### Scenario: Description in the picker
- **WHEN** the user types `/`
- **THEN** `/deploy` is listed with `ship it` as its description

#### Scenario: JSON-RPC
- **WHEN** a client calls `commands.run` with `{"name": "mv", "argv": ["a.txt", "b.txt"]}`
- **THEN** `run("a.txt", "b.txt")` is called; `{"name": "deploy", "argv": ["nope"]}` fails with `-32000`
