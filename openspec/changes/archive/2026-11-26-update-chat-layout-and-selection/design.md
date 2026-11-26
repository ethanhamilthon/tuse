## Layout and hit testing
Use the existing two-column history marker gutter as the alignment baseline. Keep the outer `pad_x` margin, message gap, and user surface styling. Apply the message inset before wrapping so the visible content, link hitboxes, and selection all use the same columns. Keep entry markers in the gutter; treat any additional Markdown or code indentation as part of the rendered content. Use the same content width in full and incremental rendering.

## Selection and copy
The current screen-row drag has no entry identity and cannot distinguish a visual wrap from a logical newline. Have the chat viewport report which visible rows belong to which selectable message and map display columns to ranges in its logical plain text. Preserve wrap boundaries and discarded word separators in that mapping. Keep synthetic padding outside the selectable ranges rather than trimming each rendered row at copy time.

Give each drag one owner: the editor or the message where it began. Clamp the moving endpoint to that owner's visible text bounds when it leaves the owner. Resolve and draw a message range only while it can still be associated with the same entry; a redraw must not transfer it to a different message. Keep the editor's existing logical offsets for mouse and keyboard selection. Leave link clicks independent of dragging.

Copy rendered text rather than raw Markdown source: preserve the visible link label and code, not styling or hidden link destinations. Copy draft ranges directly from the editor. This keeps clipboard output independent of terminal width and avoids losing intentional indentation.
