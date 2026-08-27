# Write agent stdout under phase headings

Stop wrapping watch --agent stdout in `## Agent output`. The agent must print Markdown headings that are the phase name (`## Gather`, `## Implement`, `## Verify`, or whatever the current phase is). Append stdout to the task file as it arrives so those headings are the section headers. Tests: a stub that prints two phase headings lands both in the task file; existing stdout-append tests no longer require `## Agent output`. Update README.
