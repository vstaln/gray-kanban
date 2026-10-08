//! gray-kanban — a tiny per-project kanban for the agent and the user.
//!
//! Port of hermes' kanban model (self-contained subset): one `kanban` tool
//! ({action: boards|create|add|move|done|show|rm}) plus `/kanban [board]`
//! which renders the board as text columns. Boards live under
//! `~/.gray/kanban/<cwd-hash>/<board>.json` — keyed to the session's working
//! directory so different projects get different boards.
//!
//! Columns: backlog | todo | doing | review | done. Card ids: k1, k2, …

use std::io::{BufRead, Write};
use std::path::PathBuf;

use serde_json::{Value, json};

const COLS: [&str; 5] = ["backlog", "todo", "doing", "review", "done"];
const COL_WIDTH: usize = 24;

fn manifest() -> Value {
    json!({
        "name": "kanban",
        "version": env!("CARGO_PKG_VERSION"),
        "protocol": "1.1",
        "tools": [{
            "name": "kanban",
            "description": "Manage a per-project kanban board. Actions: boards (list), create, add (needs title, optional col+note), move (id+col), done (id), show, rm (id removes a card, board removes a board). Columns: backlog|todo|doing|review|done. Card ids look like k1.",
            "parameters": {
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["boards", "create", "add", "move", "done", "show", "rm"]},
                    "board":  {"type": "string", "description": "Board name (default: main)"},
                    "title":  {"type": "string", "description": "Card title for add"},
                    "col":    {"type": "string", "description": "Column name: backlog|todo|doing|review|done"},
                    "id":     {"type": "string", "description": "Card id like k3"},
                    "note":   {"type": "string", "description": "Optional note on the card"}
                },
                "required": ["action"]
            }
        }],
        "commands": ["/kanban"],
    })
}

// ---------- storage ---------------------------------------------------------

fn state_dir() -> PathBuf {
    let home = std::env::var_os("GRAY_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".gray")))
        .unwrap_or_else(|| PathBuf::from("."));
    home.join("kanban")
}

/// FNV-1a over the session cwd — stable, dependency-free project key.
fn cwd_hash(cwd: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in cwd.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

fn boards_dir(cwd: &str) -> PathBuf {
    state_dir().join(cwd_hash(cwd))
}

fn valid_board_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && !name.starts_with('.')
}

fn board_path(cwd: &str, board: &str) -> Result<PathBuf, String> {
    if !valid_board_name(board) {
        return Err(format!("invalid board name: {board:?}"));
    }
    Ok(boards_dir(cwd).join(format!("{board}.json")))
}

fn empty_board(name: &str) -> Value {
    let mut cols = serde_json::Map::new();
    for c in COLS {
        cols.insert(c.into(), json!([]));
    }
    json!({"name": name, "next": 1, "cols": cols})
}

fn load_board(cwd: &str, board: &str) -> Result<Value, String> {
    let path = board_path(cwd, board)?;
    let text = std::fs::read_to_string(&path)
        .map_err(|_| format!("board '{board}' not found — create it with kanban {{action:\"create\",board:\"{board}\"}}"))?;
    serde_json::from_str(&text).map_err(|e| format!("board '{board}' is corrupt: {e}"))
}

fn save_board(cwd: &str, board: &str, data: &Value) -> Result<(), String> {
    let path = board_path(cwd, board)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(data).unwrap())
        .map_err(|e| format!("couldn't write board: {e}"))
}

fn valid_col(col: &str) -> bool {
    COLS.contains(&col)
}

// ---------- rendering -------------------------------------------------------

fn truncate(s: &str, width: usize) -> String {
    let mut out: String = s.chars().take(width).collect();
    if s.chars().count() > width {
        out.truncate(width.saturating_sub(1));
        out.push('…');
    }
    out
}

fn render(board: &Value) -> String {
    let name = board.get("name").and_then(Value::as_str).unwrap_or("?");
    let cols: Vec<Vec<String>> = COLS
        .iter()
        .map(|c| {
            board
                .pointer(&format!("/cols/{c}"))
                .and_then(Value::as_array)
                .map(|cards| {
                    cards
                        .iter()
                        .map(|card| {
                            let id = card.get("id").and_then(Value::as_str).unwrap_or("?");
                            let title = card.get("title").and_then(Value::as_str).unwrap_or("");
                            let note = card.get("note").and_then(Value::as_str).unwrap_or("");
                            let mut line = format!("{id} {title}");
                            if !note.is_empty() {
                                line.push_str(&format!(" — {note}"));
                            }
                            truncate(&line, COL_WIDTH - 1)
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();

    let mut out = format!("kanban: {name}\n");
    let header: Vec<String> = COLS
        .iter()
        .zip(&cols)
        .map(|(c, cards)| {
            format!(
                "{:width$}",
                format!("{} ({})", c.to_uppercase(), cards.len()),
                width = COL_WIDTH
            )
        })
        .collect();
    out.push_str(&header.join(""));
    out.push('\n');
    let rows = cols.iter().map(Vec::len).max().unwrap_or(0);
    if rows == 0 {
        out.push_str("(empty — kanban {action:\"add\",title:\"…\"} to add a card)\n");
        return out;
    }
    for row in 0..rows {
        let line: Vec<String> = cols
            .iter()
            .map(|cards| {
                let cell = cards.get(row).cloned().unwrap_or_default();
                format!("{cell:width$}", width = COL_WIDTH)
            })
            .collect();
        out.push_str(line.join("").trim_end());
        out.push('\n');
    }
    out
}

// ---------- actions ---------------------------------------------------------

fn act_boards(cwd: &str) -> Result<String, String> {
    let dir = boards_dir(cwd);
    let mut names: Vec<String> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            if let Some(name) = e.file_name().to_str() {
                if let Some(stem) = name.strip_suffix(".json") {
                    names.push(stem.to_string());
                }
            }
        }
    }
    names.sort();
    if names.is_empty() {
        Ok("no boards yet — kanban {action:\"create\"} makes 'main'".into())
    } else {
        Ok(format!("boards: {}", names.join(", ")))
    }
}

fn act_create(cwd: &str, board: &str) -> Result<String, String> {
    let path = board_path(cwd, board)?;
    if path.exists() {
        return Err(format!("board '{board}' already exists"));
    }
    save_board(cwd, board, &empty_board(board))?;
    Ok(format!("created board '{board}'"))
}

fn act_add(cwd: &str, board: &str, args: &Value) -> Result<String, String> {
    let title = args
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if title.is_empty() {
        return Err("add needs a title".into());
    }
    let col = args.get("col").and_then(Value::as_str).unwrap_or("backlog");
    if !valid_col(col) {
        return Err(format!("unknown column '{col}' — use {}", COLS.join("|")));
    }
    let mut b = if board_path(cwd, board)?.exists() {
        load_board(cwd, board)?
    } else {
        empty_board(board)
    };
    let next = b.get("next").and_then(Value::as_u64).unwrap_or(1);
    let id = format!("k{next}");
    let mut card = json!({"id": id, "title": title});
    if let Some(note) = args.get("note").and_then(Value::as_str).filter(|n| !n.is_empty()) {
        card["note"] = json!(note);
    }
    b["next"] = json!(next + 1);
    b["cols"][col].as_array_mut().unwrap().push(card);
    save_board(cwd, board, &b)?;
    Ok(format!("added {id} to {col}: {title}"))
}

fn locate_card(b: &Value, id: &str) -> Result<(&'static str, usize), String> {
    for c in COLS {
        if let Some(cards) = b["cols"][c].as_array() {
            if let Some(pos) = cards.iter().position(|k| k["id"].as_str() == Some(id)) {
                return Ok((c, pos));
            }
        }
    }
    Err(format!("no card '{id}' on this board"))
}

fn act_move(cwd: &str, board: &str, args: &Value, target: Option<&str>) -> Result<String, String> {
    let id = args.get("id").and_then(Value::as_str).unwrap_or("").trim();
    if id.is_empty() {
        return Err("move/done needs a card id".into());
    }
    let col = target
        .map(str::to_string)
        .or_else(|| args.get("col").and_then(Value::as_str).map(str::to_string))
        .ok_or_else(|| "move needs a target col".to_string())?;
    if !valid_col(&col) {
        return Err(format!("unknown column '{col}' — use {}", COLS.join("|")));
    }
    let mut b = load_board(cwd, board)?;
    let (src, pos) = locate_card(&b, id)?;
    if src == col {
        return Ok(format!("{id} is already in {col}"));
    }
    let card = b["cols"][src][pos].clone();
    b["cols"][src].as_array_mut().unwrap().remove(pos);
    b["cols"][&col].as_array_mut().unwrap().push(card.clone());
    save_board(cwd, board, &b)?;
    let title = card.get("title").and_then(Value::as_str).unwrap_or("");
    Ok(format!("moved {id} {src} → {col}: {title}"))
}

fn act_rm(cwd: &str, board: &str, args: &Value) -> Result<String, String> {
    if let Some(id) = args.get("id").and_then(Value::as_str).filter(|s| !s.is_empty()) {
        let mut b = load_board(cwd, board)?;
        let (src, pos) = locate_card(&b, id)?;
        let card = b["cols"][src][pos].clone();
        b["cols"][src].as_array_mut().unwrap().remove(pos);
        save_board(cwd, board, &b)?;
        let title = card.get("title").and_then(Value::as_str).unwrap_or("");
        return Ok(format!("removed {id}: {title}"));
    }
    let path = board_path(cwd, board)?;
    if !path.exists() {
        return Err(format!("board '{board}' not found"));
    }
    std::fs::remove_file(&path).map_err(|e| format!("couldn't remove board: {e}"))?;
    Ok(format!("removed board '{board}'"))
}

fn session_cwd(params: &Value) -> String {
    params
        .pointer("/session/cwd")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| std::env::var("PWD").ok())
        .unwrap_or_else(|| ".".into())
}

fn call_tool(name: &str, args: &Value, session: &Value) -> Result<String, String> {
    if name != "kanban" {
        return Err(format!("unknown tool: {name}"));
    }
    let cwd = session_cwd(&json!({ "session": session }));
    let action = args.get("action").and_then(Value::as_str).unwrap_or("show");
    let board = args.get("board").and_then(Value::as_str).unwrap_or("main");
    match action {
        "boards" => act_boards(&cwd),
        "create" => act_create(&cwd, board),
        "add" => act_add(&cwd, board, args),
        "move" => act_move(&cwd, board, args, None),
        "done" => act_move(&cwd, board, args, Some("done")),
        "show" => Ok(render(&load_board(&cwd, board)?)),
        "rm" => act_rm(&cwd, board, args),
        other => Err(format!("unknown action '{other}' — boards|create|add|move|done|show|rm")),
    }
}

/// `/kanban [board]` — render the board as text columns for the user.
fn run_command(argv: &[&str], session: &Value) -> String {
    let cwd = session_cwd(&json!({ "session": session }));
    match argv.first().copied() {
        Some("boards") => act_boards(&cwd).unwrap_or_else(|e| e),
        Some(board) => match load_board(&cwd, board) {
            Ok(b) => render(&b),
            Err(e) => e,
        },
        None => match load_board(&cwd, "main") {
            Ok(b) => render(&b),
            Err(e) => format!(
                "{e}\n\n/kanban [board] renders a board · kanban tool actions: boards|create|add|move|done|show|rm"
            ),
        },
    }
}

/// One request → `Some(reply)`, or `None` for notifications. The bool asks
/// the loop to exit after writing the reply.
fn handle(req: &Value) -> (Option<Value>, bool) {
    let id = req.get("id").cloned();
    let method = req.get("method").and_then(Value::as_str).unwrap_or("");
    let params = req.get("params").cloned().unwrap_or(Value::Null);
    let Some(id) = id else {
        return (None, method == "plugin/shutdown");
    };
    let result = match method {
        "plugin/manifest" => manifest(),
        "tool/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("args").cloned().unwrap_or(Value::Null);
            let session = params.get("session").cloned().unwrap_or(Value::Null);
            match call_tool(name, &args, &session) {
                Ok(text) => json!({ "content": text }),
                Err(e) => json!({ "content": e, "is_error": true }),
            }
        }
        "command/run" => {
            let argv: Vec<&str> = params
                .get("argv")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            let session = params.get("session").cloned().unwrap_or(Value::Null);
            json!({ "text": run_command(&argv, &session) })
        }
        "plugin/shutdown" => return (Some(json!({ "id": id, "result": {} })), true),
        _ => {
            let error = json!({ "code": -32601, "message": "method not found" });
            return (Some(json!({ "id": id, "error": error })), false);
        }
    };
    (Some(json!({ "id": id, "result": result })), false)
}

fn main() -> std::io::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("manifest") {
        println!("{}", manifest());
        return Ok(());
    }
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        let Ok(req) = serde_json::from_str::<Value>(&line) else { continue };
        let (reply, exit) = handle(&req);
        if let Some(reply) = reply {
            writeln!(stdout, "{reply}")?;
            stdout.flush()?;
        }
        if exit {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    struct TempHome;
    impl TempHome {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("gray-kanban-test-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            unsafe { std::env::set_var("GRAY_HOME", &dir) };
            Self
        }
    }
    impl Drop for TempHome {
        fn drop(&mut self) {
            let dir = std::env::temp_dir().join(format!("gray-kanban-test-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    fn call(method: &str, params: Value) -> Value {
        handle(&json!({ "id": 1, "method": method, "params": params })).0.unwrap()
    }

    fn tool(args: Value) -> Value {
        call(
            "tool/call",
            json!({ "name": "kanban", "args": args, "session": {"cwd": "/tmp/proj"} }),
        )["result"]
            .clone()
    }

    #[test]
    fn manifest_shape() {
        let m = call("plugin/manifest", Value::Null)["result"].clone();
        assert_eq!(m["name"], "kanban");
        assert_eq!(m["commands"], json!(["/kanban"]));
        assert_eq!(m["tools"][0]["name"], "kanban");
    }

    #[test]
    fn add_move_done_show() {
        let _g = LOCK.lock().unwrap();
        let _h = TempHome::new();
        let r = tool(json!({"action": "add", "title": "write tests"}));
        assert!(r["content"].as_str().unwrap().contains("k1"), "{r}");
        let r = tool(json!({"action": "move", "id": "k1", "col": "doing"}));
        assert!(r["content"].as_str().unwrap().contains("backlog → doing"));
        let r = tool(json!({"action": "done", "id": "k1"}));
        assert!(r["content"].as_str().unwrap().contains("→ done"));
        let r = tool(json!({"action": "show"}));
        let text = r["content"].as_str().unwrap();
        assert!(text.contains("DONE (1)") && text.contains("write tests"), "{text}");
    }

    #[test]
    fn errors_are_tool_errors() {
        let _g = LOCK.lock().unwrap();
        let _h = TempHome::new();
        assert_eq!(tool(json!({"action": "add"}))["is_error"], true);
        assert_eq!(tool(json!({"action": "show", "board": "nope"}))["is_error"], true);
        assert_eq!(
            tool(json!({"action": "move", "id": "k9", "col": "mars"}))["is_error"],
            true
        );
        assert_eq!(tool(json!({"action": "bogus"}))["is_error"], true);
    }

    #[test]
    fn boards_and_rm() {
        let _g = LOCK.lock().unwrap();
        let _h = TempHome::new();
        tool(json!({"action": "add", "title": "x", "board": "bugs"}));
        let r = tool(json!({"action": "boards"}));
        assert!(r["content"].as_str().unwrap().contains("bugs"), "{r}");
        let r = tool(json!({"action": "rm", "board": "bugs"}));
        assert!(r["content"].as_str().unwrap().contains("removed board"));
    }

    #[test]
    fn board_name_traversal_rejected() {
        assert!(board_path("/tmp/proj", "../evil").is_err());
        assert!(board_path("/tmp/proj", "a/b").is_err());
        assert!(board_path("/tmp/proj", "main").is_ok());
    }

    #[test]
    fn command_renders_board() {
        let _g = LOCK.lock().unwrap();
        let _h = TempHome::new();
        tool(json!({"action": "add", "title": "ship it"}));
        let r = call(
            "command/run",
            json!({ "name": "/kanban", "argv": [], "session": {"cwd": "/tmp/proj"} }),
        );
        let text = r["result"]["text"].as_str().unwrap();
        assert!(text.contains("kanban: main") && text.contains("ship it"), "{text}");
    }

    #[test]
    fn different_cwds_get_different_boards() {
        let _g = LOCK.lock().unwrap();
        let _h = TempHome::new();
        call(
            "tool/call",
            json!({ "name": "kanban", "args": {"action": "add", "title": "a"},
                    "session": {"cwd": "/tmp/one"} }),
        );
        let r = call(
            "tool/call",
            json!({ "name": "kanban", "args": {"action": "show"},
                    "session": {"cwd": "/tmp/two"} }),
        );
        assert_eq!(r["result"]["is_error"], true);
    }

    #[test]
    fn shutdown_replies_then_exits() {
        let (reply, exit) = handle(&json!({ "id": 2, "method": "plugin/shutdown" }));
        assert!(reply.is_some() && exit);
    }
}
