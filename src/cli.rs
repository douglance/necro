use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;

use async_trait::async_trait;
use futures::Stream;
use incurs::cli::Cli;
use incurs::command::{
    CommandContext, CommandDef, CommandHandler, Example, McpAnnotations, McpCommandOptions,
    TypedContext, TypedResult,
};
use incurs::output::{CommandResult, CtaBlock, CtaEntry};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::board::{self, InitResult};
use crate::error::Error;
use crate::store::Store;
use crate::task::{Lane, StatusFilter, TaskRecord, TaskSummary};
use crate::watch::{self, AgentKind, WatchFilter, WatchKind};

#[derive(Debug, Default, Deserialize, incurs::Options)]
struct RootOptions {
    /// Board directory. Default: walk up from the working directory.
    root: Option<PathBuf>,
}

#[derive(Debug, Deserialize, incurs::Env)]
struct RootEnv {
    /// Board directory override.
    #[incurs(env = "NECRO_ROOT")]
    necro_root: Option<PathBuf>,
}

#[derive(Debug, Deserialize, incurs::Args)]
struct AddArgs {
    /// Task title used for the heading and slug.
    title: String,
}

#[derive(Debug, Deserialize, incurs::Options)]
struct AddOptions {
    /// Optional body appended after the title heading.
    body: Option<String>,
}

#[derive(Debug, Deserialize, incurs::Options)]
struct ListOptions {
    /// Filter by lane: `todo`, `doing`, `done`, `dropped`, or `all`.
    #[incurs(alias = "s", default = "todo")]
    status: String,
}

#[derive(Debug, Deserialize, incurs::Args)]
struct IdArgs {
    /// Task ID (filename stem).
    id: String,
}

#[derive(Debug, Deserialize, incurs::Args)]
struct NoteArgs {
    /// Task ID (filename stem).
    id: String,
    /// Markdown text to append.
    text: String,
}

#[derive(Debug, Deserialize, incurs::Options)]
struct NextOptions {
    /// Move the selected todo into doing.
    #[incurs(default = false)]
    start: bool,
}

#[derive(Debug, JsonSchema, Serialize)]
struct ListOutput {
    tasks: Vec<TaskSummary>,
}

#[derive(Debug, JsonSchema, Serialize)]
struct NextOutput {
    task: Option<TaskRecord>,
}

#[derive(Debug, Deserialize, incurs::Options)]
struct WatchOptions {
    /// Restrict events to one lane: `todo`, `doing`, `done`, or `dropped`.
    lane: Option<String>,
    /// `enter`, `exit`, or `all`.
    #[incurs(default = "enter")]
    #[serde(default = "default_watch_event")]
    event: String,
    /// Wait for one matching event and exit.
    #[incurs(default = false)]
    #[serde(default)]
    once: bool,
    /// Emit enter events for files already present when watch starts.
    #[incurs(default = false)]
    #[serde(default)]
    existing: bool,
    /// Shell command to run for each matching event.
    exec: Option<String>,
    /// Headless agent to spawn per event (`grok` runs `-p` with `--always-approve`; `claude` runs `-p` with `--dangerously-skip-permissions`). Stdout is appended to the task file.
    #[serde(default)]
    agent: Option<String>,
    /// Poll interval in milliseconds.
    #[incurs(default = 250)]
    #[serde(default = "default_interval_ms")]
    interval_ms: u64,
}

fn default_watch_event() -> String {
    "enter".to_string()
}

fn default_interval_ms() -> u64 {
    250
}

/// Builds the shared incurs command graph used by CLI and MCP.
pub fn build_cli() -> Cli {
    Cli::create("necro")
        .description("File-board task manager for agents.")
        .version(env!("CARGO_PKG_VERSION"))
        .globals::<RootOptions>()
        .command("init", init_command())
        .command("add", add_command())
        .command("list", list_command())
        .command("snapshot", snapshot_command())
        .command("next", next_command())
        .command("show", show_command())
        .command("note", note_command())
        .command("start", start_command())
        .command("done", done_command())
        .command("drop", drop_command())
        .command("reopen", reopen_command())
        .command("watch", watch_command())
}

fn init_command() -> CommandDef {
    CommandDef::typed::<(), (), RootEnv, InitResult, _, _>(
        "init",
        |ctx: TypedContext<(), (), RootEnv>| async move {
            match init_board(&ctx) {
                Ok(output) => TypedResult::ok(output),
                Err(error) => fail(error),
            }
        },
    )
    .description("Create .necro lane directories under the chosen root.")
    .examples(vec![Example {
        command: String::new(),
        description: Some("Create a board in the current directory".to_string()),
    }])
    .mcp(write_tool("Init a necro board", true))
    .done()
}

fn add_command() -> CommandDef {
    CommandDef::typed::<AddArgs, AddOptions, RootEnv, TaskRecord, _, _>(
        "add",
        |ctx: TypedContext<AddArgs, AddOptions, RootEnv>| async move {
            match add_task(&ctx) {
                Ok(output) => TypedResult::ok_with_cta(
                    output.clone(),
                    CtaBlock {
                        description: Some("Next:".to_string()),
                        commands: vec![
                            CtaEntry::Simple("list".to_string()),
                            CtaEntry::Detailed {
                                command: format!("show {}", output.id),
                                description: Some("Read the task".to_string()),
                            },
                        ],
                    },
                ),
                Err(error) => fail(error),
            }
        },
    )
    .description("Create a Markdown task in .necro/1-todo.")
    .examples(vec![Example {
        command: "\"Fix auth refresh\"".to_string(),
        description: Some("Add an open task".to_string()),
    }])
    .mcp(write_tool("Add a necro task", false))
    .done()
}

fn list_command() -> CommandDef {
    CommandDef::typed::<(), ListOptions, RootEnv, ListOutput, _, _>(
        "list",
        |ctx: TypedContext<(), ListOptions, RootEnv>| async move {
            match list_tasks(&ctx) {
                Ok(output) => TypedResult::ok(output),
                Err(error) => fail(error),
            }
        },
    )
    .description("List tasks. Defaults to the todo lane.")
    .examples(vec![
        Example {
            command: String::new(),
            description: Some("List open tasks".to_string()),
        },
        Example {
            command: "--status doing".to_string(),
            description: Some("List in-progress tasks".to_string()),
        },
        Example {
            command: "--status done".to_string(),
            description: Some("List finished tasks".to_string()),
        },
        Example {
            command: "--status dropped".to_string(),
            description: Some("List dropped tasks".to_string()),
        },
    ])
    .mcp(read_tool("List necro tasks"))
    .done()
}

fn snapshot_command() -> CommandDef {
    CommandDef::typed::<(), (), RootEnv, ListOutput, _, _>(
        "snapshot",
        |ctx: TypedContext<(), (), RootEnv>| async move {
            match snapshot_tasks(&ctx) {
                Ok(output) => TypedResult::ok(output),
                Err(error) => fail(error),
            }
        },
    )
    .description("List every task in todo, doing, done, and dropped.")
    .examples(vec![Example {
        command: String::new(),
        description: Some("Show the whole board".to_string()),
    }])
    .mcp(read_tool("Snapshot necro lanes"))
    .done()
}

fn next_command() -> CommandDef {
    CommandDef::typed::<(), NextOptions, RootEnv, NextOutput, _, _>(
        "next",
        |ctx: TypedContext<(), NextOptions, RootEnv>| async move {
            match next_task(&ctx) {
                Ok(output) => {
                    let cta = next_cta(&output, ctx.options.start);
                    TypedResult::ok_with_cta(output, cta)
                }
                Err(error) => fail(error),
            }
        },
    )
    .description("Return the next open todo, optionally starting it.")
    .examples(vec![
        Example {
            command: String::new(),
            description: Some("Show the next todo".to_string()),
        },
        Example {
            command: "--start".to_string(),
            description: Some("Start the next todo".to_string()),
        },
    ])
    .mcp(write_tool("Select the next necro task", false))
    .done()
}

fn show_command() -> CommandDef {
    CommandDef::typed::<IdArgs, (), RootEnv, TaskRecord, _, _>(
        "show",
        |ctx: TypedContext<IdArgs, (), RootEnv>| async move {
            match show_task(&ctx) {
                Ok(output) => TypedResult::ok(output),
                Err(error) => fail(error),
            }
        },
    )
    .description("Read one task by ID.")
    .examples(vec![Example {
        command: "fix-auth-refresh".to_string(),
        description: Some("Show a task".to_string()),
    }])
    .mcp(read_tool("Show a necro task"))
    .done()
}

fn note_command() -> CommandDef {
    CommandDef::typed::<NoteArgs, (), RootEnv, TaskRecord, _, _>(
        "note",
        |ctx: TypedContext<NoteArgs, (), RootEnv>| async move {
            match note_task(&ctx) {
                Ok(output) => TypedResult::ok_with_cta(
                    output.clone(),
                    CtaBlock {
                        description: Some("Next:".to_string()),
                        commands: vec![CtaEntry::Detailed {
                            command: format!("show {}", output.id),
                            description: Some("Read the task".to_string()),
                        }],
                    },
                ),
                Err(error) => fail(error),
            }
        },
    )
    .description("Append Markdown text to a task without changing status.")
    .examples(vec![Example {
        command: "fix-auth-refresh \"Checked the token refresh path.\"".to_string(),
        description: Some("Append a note".to_string()),
    }])
    .mcp(write_tool("Append a necro note", false))
    .done()
}

fn start_command() -> CommandDef {
    CommandDef::typed::<IdArgs, (), RootEnv, TaskRecord, _, _>(
        "start",
        |ctx: TypedContext<IdArgs, (), RootEnv>| async move {
            match start_task(&ctx) {
                Ok(output) => TypedResult::ok_with_cta(
                    output,
                    CtaBlock {
                        description: Some("Next:".to_string()),
                        commands: vec![CtaEntry::Simple("list --status doing".to_string())],
                    },
                ),
                Err(error) => fail(error),
            }
        },
    )
    .description("Move a task from .necro/1-todo to .necro/2-doing.")
    .examples(vec![Example {
        command: "fix-auth-refresh".to_string(),
        description: Some("Start a task".to_string()),
    }])
    .mcp(write_tool("Start a necro task", true))
    .done()
}

fn done_command() -> CommandDef {
    CommandDef::typed::<IdArgs, (), RootEnv, TaskRecord, _, _>(
        "done",
        |ctx: TypedContext<IdArgs, (), RootEnv>| async move {
            match move_task(&ctx, true) {
                Ok(output) => TypedResult::ok_with_cta(
                    output,
                    CtaBlock {
                        description: Some("Next:".to_string()),
                        commands: vec![CtaEntry::Simple("list --status done".to_string())],
                    },
                ),
                Err(error) => fail(error),
            }
        },
    )
    .description("Move a task from .necro/1-todo or .necro/2-doing to .necro/3-done.")
    .examples(vec![Example {
        command: "fix-auth-refresh".to_string(),
        description: Some("Mark a task done".to_string()),
    }])
    .mcp(write_tool("Complete a necro task", true))
    .done()
}

fn drop_command() -> CommandDef {
    CommandDef::typed::<IdArgs, (), RootEnv, TaskRecord, _, _>(
        "drop",
        |ctx: TypedContext<IdArgs, (), RootEnv>| async move {
            match drop_task(&ctx) {
                Ok(output) => TypedResult::ok_with_cta(
                    output,
                    CtaBlock {
                        description: Some("Next:".to_string()),
                        commands: vec![CtaEntry::Simple("list --status dropped".to_string())],
                    },
                ),
                Err(error) => fail(error),
            }
        },
    )
    .description("Move a task to .necro/4-dropped.")
    .examples(vec![Example {
        command: "fix-auth-refresh".to_string(),
        description: Some("Drop a task".to_string()),
    }])
    .mcp(write_tool("Drop a necro task", true))
    .done()
}

fn reopen_command() -> CommandDef {
    CommandDef::typed::<IdArgs, (), RootEnv, TaskRecord, _, _>(
        "reopen",
        |ctx: TypedContext<IdArgs, (), RootEnv>| async move {
            match move_task(&ctx, false) {
                Ok(output) => TypedResult::ok_with_cta(
                    output,
                    CtaBlock {
                        description: Some("Next:".to_string()),
                        commands: vec![CtaEntry::Simple("list".to_string())],
                    },
                ),
                Err(error) => fail(error),
            }
        },
    )
    .description("Move a task back to .necro/1-todo.")
    .examples(vec![Example {
        command: "fix-auth-refresh".to_string(),
        description: Some("Reopen a finished task".to_string()),
    }])
    .mcp(write_tool("Reopen a necro task", true))
    .done()
}

fn watch_command() -> CommandDef {
    CommandDef::build("watch", WatchHandler)
        .description("Watch lane folders and run a command or headless agent when a task enters or exits.")
        .options::<WatchOptions>()
        .env::<RootEnv>()
        .examples(vec![
            Example {
                command: "--once --existing --lane todo".to_string(),
                description: Some(
                    "Wait for the next todo, including files already there".to_string(),
                ),
            },
            Example {
                command: "--lane todo --agent grok".to_string(),
                description: Some("Spawn a headless grok agent for each new todo".to_string()),
            },
            Example {
                command: "--lane todo --agent claude".to_string(),
                description: Some("Spawn a headless claude agent for each new todo".to_string()),
            },
            Example {
                command: "--lane todo --exec 'necro start \"$NECRO_ID\"'".to_string(),
                description: Some("Start each new todo".to_string()),
            },
        ])
        .hint("Sets NECRO_EVENT, NECRO_LANE, NECRO_ID, NECRO_PATH, and NECRO_ROOT for --exec and --agent.")
        .mcp(McpCommandOptions {
            annotations: Some(McpAnnotations {
                title: Some("Watch necro lanes".to_string()),
                read_only_hint: Some(false),
                idempotent_hint: Some(false),
                destructive_hint: Some(false),
                open_world_hint: Some(true),
            }),
            ..McpCommandOptions::default()
        })
        .done()
}

struct WatchHandler;

#[async_trait]
impl CommandHandler for WatchHandler {
    async fn run(&self, ctx: CommandContext) -> CommandResult {
        match run_watch(ctx).await {
            Ok(result) => result,
            Err(error) => CommandResult::Error {
                code: error.code().to_string(),
                message: error.to_string(),
                retryable: false,
                exit_code: Some(1),
                cta: None,
            },
        }
    }
}

async fn run_watch(ctx: CommandContext) -> Result<CommandResult, Error> {
    let options: WatchOptions =
        serde_json::from_value(ctx.options.clone()).unwrap_or(WatchOptions {
            lane: None,
            event: default_watch_event(),
            once: false,
            existing: false,
            exec: None,
            agent: None,
            interval_ms: default_interval_ms(),
        });
    let env: RootEnv =
        serde_json::from_value(ctx.env.clone()).unwrap_or(RootEnv { necro_root: None });
    let root = {
        let explicit = explicit_root(ctx.globals.clone(), &env)?;
        let cwd = std::env::current_dir()?;
        board::require_board(explicit.as_deref(), &cwd)?
    };
    let filter = WatchFilter {
        lanes: match options.lane {
            Some(name) => vec![Lane::parse(&name).ok_or(Error::InvalidStatus(name))?],
            None => Vec::new(),
        },
        kinds: WatchKind::parse_filter(&options.event)?.to_vec(),
    };
    let interval = Duration::from_millis(options.interval_ms.max(10));
    if options.agent.is_some() && options.exec.is_some() {
        return Err(Error::AgentAndExec);
    }
    let agent = options.agent.as_deref().map(AgentKind::parse).transpose()?;
    let lock = if agent.is_some() {
        Some(watch::AgentWatchLock::acquire(&root)?)
    } else {
        None
    };
    if options.once {
        let event = watch::wait_once(
            root,
            filter,
            options.existing,
            interval,
            options.exec,
            agent,
        )
        .await?;
        drop(lock);
        let data = serde_json::to_value(&event)
            .map_err(|error| Error::Io(std::io::Error::other(error.to_string())))?;
        return Ok(CommandResult::Ok {
            data,
            cta: Some(CtaBlock {
                description: Some("Next:".to_string()),
                commands: vec![CtaEntry::Detailed {
                    command: format!("show {}", event.id),
                    description: Some("Read the task".to_string()),
                }],
            }),
            exit_code: None,
        });
    }
    let stream: Pin<Box<dyn Stream<Item = Value> + Send>> = Box::pin(watch::watch_stream(
        root,
        filter,
        options.existing,
        interval,
        options.exec,
        agent,
        lock,
    ));
    Ok(CommandResult::Stream(stream))
}

fn init_board(ctx: &TypedContext<(), (), RootEnv>) -> Result<InitResult, Error> {
    let root = chosen_root(ctx.globals.clone(), &ctx.env)?;
    board::init(&root)
}

fn add_task(ctx: &TypedContext<AddArgs, AddOptions, RootEnv>) -> Result<TaskRecord, Error> {
    let store = open_store(ctx.globals.clone(), &ctx.env)?;
    store.add(&ctx.args.title, ctx.options.body.as_deref())
}

fn next_task(ctx: &TypedContext<(), NextOptions, RootEnv>) -> Result<NextOutput, Error> {
    let store = open_store(ctx.globals.clone(), &ctx.env)?;
    let Some(task) = store.next()? else {
        return Ok(NextOutput { task: None });
    };
    if ctx.options.start {
        return Ok(NextOutput {
            task: Some(store.start(&task.id)?),
        });
    }
    Ok(NextOutput { task: Some(task) })
}

fn next_cta(output: &NextOutput, started: bool) -> CtaBlock {
    match &output.task {
        None => CtaBlock {
            description: Some("No open tasks:".to_string()),
            commands: vec![CtaEntry::Simple("add".to_string())],
        },
        Some(task) if started => CtaBlock {
            description: Some("Next:".to_string()),
            commands: vec![CtaEntry::Detailed {
                command: format!("show {}", task.id),
                description: Some("Read the task".to_string()),
            }],
        },
        Some(task) => CtaBlock {
            description: Some("Next:".to_string()),
            commands: vec![
                CtaEntry::Detailed {
                    command: format!("start {}", task.id),
                    description: Some("Start the task".to_string()),
                },
                CtaEntry::Detailed {
                    command: format!("show {}", task.id),
                    description: Some("Read the task".to_string()),
                },
            ],
        },
    }
}

fn list_tasks(ctx: &TypedContext<(), ListOptions, RootEnv>) -> Result<ListOutput, Error> {
    let store = open_store(ctx.globals.clone(), &ctx.env)?;
    let filter = StatusFilter::parse(&ctx.options.status)
        .ok_or_else(|| Error::InvalidStatus(ctx.options.status.clone()))?;
    Ok(ListOutput {
        tasks: store.list(filter)?,
    })
}

fn snapshot_tasks(ctx: &TypedContext<(), (), RootEnv>) -> Result<ListOutput, Error> {
    let store = open_store(ctx.globals.clone(), &ctx.env)?;
    Ok(ListOutput {
        tasks: store.snapshot()?,
    })
}

fn show_task(ctx: &TypedContext<IdArgs, (), RootEnv>) -> Result<TaskRecord, Error> {
    let store = open_store(ctx.globals.clone(), &ctx.env)?;
    store.show(&ctx.args.id)
}

fn note_task(ctx: &TypedContext<NoteArgs, (), RootEnv>) -> Result<TaskRecord, Error> {
    let store = open_store(ctx.globals.clone(), &ctx.env)?;
    store.note(&ctx.args.id, &ctx.args.text)
}

fn start_task(ctx: &TypedContext<IdArgs, (), RootEnv>) -> Result<TaskRecord, Error> {
    let store = open_store(ctx.globals.clone(), &ctx.env)?;
    store.start(&ctx.args.id)
}

fn drop_task(ctx: &TypedContext<IdArgs, (), RootEnv>) -> Result<TaskRecord, Error> {
    let store = open_store(ctx.globals.clone(), &ctx.env)?;
    store.drop(&ctx.args.id)
}

fn move_task(ctx: &TypedContext<IdArgs, (), RootEnv>, done: bool) -> Result<TaskRecord, Error> {
    let store = open_store(ctx.globals.clone(), &ctx.env)?;
    if done {
        store.done(&ctx.args.id)
    } else {
        store.reopen(&ctx.args.id)
    }
}

fn open_store(globals: serde_json::Value, env: &RootEnv) -> Result<Store, Error> {
    let explicit = explicit_root(globals, env)?;
    let cwd = std::env::current_dir()?;
    let root = board::require_board(explicit.as_deref(), &cwd)?;
    Store::open(root)
}

fn chosen_root(globals: serde_json::Value, env: &RootEnv) -> Result<PathBuf, Error> {
    let explicit = explicit_root(globals, env)?;
    let cwd = std::env::current_dir()?;
    board::chosen_root(explicit.as_deref(), &cwd)
}

fn explicit_root(globals: serde_json::Value, env: &RootEnv) -> Result<Option<PathBuf>, Error> {
    let options: RootOptions = serde_json::from_value(globals).unwrap_or_default();
    Ok(options.root.or_else(|| env.necro_root.clone()))
}

fn fail<T>(error: Error) -> TypedResult<T> {
    let cta = match error {
        Error::BoardNotFound(_) => Some(CtaBlock {
            description: Some("Create a board:".to_string()),
            commands: vec![CtaEntry::Simple("init".to_string())],
        }),
        Error::TaskNotFound(_) => Some(CtaBlock {
            description: Some("List open tasks:".to_string()),
            commands: vec![CtaEntry::Simple("list".to_string())],
        }),
        _ => None,
    };
    TypedResult::Error {
        code: error.code().to_string(),
        message: error.to_string(),
        retryable: false,
        exit_code: Some(1),
        cta,
    }
}

fn read_tool(title: &str) -> McpCommandOptions {
    McpCommandOptions {
        annotations: Some(McpAnnotations {
            title: Some(title.to_string()),
            read_only_hint: Some(true),
            idempotent_hint: Some(true),
            destructive_hint: Some(false),
            open_world_hint: Some(false),
        }),
        ..McpCommandOptions::default()
    }
}

fn write_tool(title: &str, idempotent: bool) -> McpCommandOptions {
    McpCommandOptions {
        annotations: Some(McpAnnotations {
            title: Some(title.to_string()),
            read_only_hint: Some(false),
            idempotent_hint: Some(idempotent),
            destructive_hint: Some(false),
            open_world_hint: Some(false),
        }),
        ..McpCommandOptions::default()
    }
}
