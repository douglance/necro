# Stream agent stdout into the task file live

Pipe watch --agent stdout into the task markdown as it arrives, not only after exit. Follow the file if it moves. Keep a ## Agent output heading. Tests must observe partial output in the file before the stub process exits.
