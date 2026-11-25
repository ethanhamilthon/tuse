## ADDED Requirements

### Requirement: Wake the TUI on streaming updates
The TUI SHALL wake when session events arrive, without waiting for terminal input or a periodic poll. The first displayable streaming update of each model attempt MUST be drawn at the next render opportunity, without waiting for a frame-rate timer, a later delta, or the final reply. Later updates MAY share a frame, but the last pending update MUST be drawn even if the stream pauses. Stream processing MUST preserve event order and leave opportunities to handle terminal input.

#### Scenario: First delta in an idle terminal
- **WHEN** a provider sends one displayable delta, then holds the connection open without sending another event, and the user does not press a key
- **THEN** the TUI draws the delta without waiting for the frame pacing deadline or the response to finish

#### Scenario: Burst followed by a pause
- **WHEN** many deltas arrive, the user presses a key, and then the provider pauses
- **THEN** the TUI handles input during the burst and renders the accumulated content in order, including the last delta, without waiting for another event

### Requirement: Show the foreground activity in the composer
Both input styles SHALL distinguish provider wait with the existing spinner, foreground Lua execution with the Lua block's animated star, and model streaming with a fixed-width dot that pulses through progressively larger and smaller glyphs. The elapsed timer SHALL remain available with the activity. Streaming SHALL begin at the first displayable text, reasoning, or Lua code update; empty announcements alone MUST NOT activate the pulse or the Lua execution star. A pause between chunks MUST NOT switch the indicator back to waiting. Activity changes MUST NOT insert rows into the chat. The TUI SHALL preserve steering and retry notices and MUST NOT keep showing an old retry notice over a new attempt. Ending, stopping, or switching sessions MUST clear stale activity.

#### Scenario: Wait, stream, run Lua
- **WHEN** the model request is sent, the provider announces a tool without code, then sends displayable code, and the foreground Lua function starts after the reply
- **THEN** the spinner remains through the empty announcement, the dot pulses while code streams, and the indicator changes to a star only when Lua executes

#### Scenario: Reasoning and text arrive intermittently
- **WHEN** displayable reasoning or reply text arrives and the provider pauses mid-response
- **THEN** the pulse begins with the first displayed content and continues through the pause without changing the composer height

#### Scenario: Retry and stop
- **WHEN** a partial response is retried and the new attempt is then stopped
- **THEN** the retry notice replaces stale activity during the wait for retry, the provider spinner returns for the new attempt, and stopping clears its indicator

### Requirement: Show the Lua preview only with code
The TUI MUST NOT reserve a chat row or show an inline Lua star for a tool announcement or whitespace-only code. The inline star and the first displayable Lua code MUST appear together in the same frame, whether the code first arrives in a delta or at tool start. A hidden preview discarded on reset, cancellation, or session switch MUST NOT flash on screen.

#### Scenario: Metadata and whitespace arrive before code
- **WHEN** the provider announces a Lua call, sends whitespace-only code, and then sends its first displayable code chunk
- **THEN** the announcement and whitespace create no chat row, gap, or star, and the inline star first appears beside code in the same frame

#### Scenario: No displayable code streams
- **WHEN** a Lua call has no displayable streaming delta but its tool start carries displayable code
- **THEN** the TUI shows the inline star and code together at tool start, without a prior placeholder

#### Scenario: Reset before code arrives
- **WHEN** the attempt is reset before a pending Lua preview gets displayable code
- **THEN** the pending preview is discarded without ever appearing in chat

### Requirement: Boxed input composer
When `settings.ui.input_style` is `boxed` and the viewport fits, the composer SHALL occupy exactly five terminal rows: a top border, two editor rows, a status row inside the border, and a bottom border. Side borders SHALL enclose the three interior rows. The status row SHALL place activity and notices on the left and the model/context label on the right; labels MUST be clipped rather than overlap. A longer draft SHALL scroll inside the editor area with the cursor visible. `input_max_height` MAY limit the visible draft rows but MUST NOT expand or shrink the box. The classic composer SHALL be used when the box cannot fit, without changing the configured style or losing the draft. Both styles SHALL retain editing, completion, mouse and selection behavior.

#### Scenario: Boxed composer with a multiline draft
- **WHEN** boxed input is configured in a terminal with sufficient space and the draft wraps to more than two rows
- **THEN** the five-row box stays fixed, the editor scrolls to keep the cursor visible, and the status row keeps activity on the left and model/context on the right

#### Scenario: Resize or reload
- **WHEN** the terminal becomes too small for the box, grows again, or `/reload` changes the input style
- **THEN** the layout and input hit testing update together, and the draft and logical cursor position are preserved
