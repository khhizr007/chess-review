//! An optional human-move model: how likely a player of a given rating is to
//! play a move (`docs/coach-analysis.md` §6.7). The coach uses it to say when
//! the best move was hard to find at the player's level, and when a bad move
//! was a natural one.
//!
//! The model is Maia-2 (MIT), run by its reference Python implementation in
//! a sidecar process (`tools/maia/maia_sidecar.py`), started from
//! `MAIA_COMMAND`. It speaks one JSON object per line on stdin and stdout:
//!
//! ```text
//! <- {"ready": true, "name": "Maia-2 rapid"}
//! -> {"queries": [{"fen": "...", "moves": ["e2e4", "d2d4"], "elo_self": 1200, "elo_oppo": 1250}]}
//! <- {"odds": [[0.41, 0.22]]}            (or {"error": "..."})
//! ```
//!
//! Without the sidecar the review runs as before and the coach's wording does
//! not change.

use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::time::timeout;

/// Loading the model may take a while (the first start reads its weights).
const START_TIMEOUT: Duration = Duration::from_secs(120);
/// One batch of queries.
const BATCH_TIMEOUT: Duration = Duration::from_secs(120);

/// One position: the chance of each of `moves` (UCI) for a player of
/// `elo_self` against `elo_oppo`, the side to move playing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MoveQuery {
    pub fen: String,
    pub moves: Vec<String>,
    pub elo_self: u32,
    pub elo_oppo: u32,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum HumanModelError {
    #[error("cannot start the human-move model: {0}")]
    Spawn(String),
    #[error("the human-move model failed: {0}")]
    Failed(String),
}

#[async_trait]
pub trait HumanModel: Send + Sync {
    /// The chance of each query's moves, in order (0 for a move it does not
    /// consider).
    async fn odds(&self, queries: &[MoveQuery]) -> Result<Vec<Vec<f32>>, HumanModelError>;

    fn name(&self) -> String;
}

#[derive(Deserialize)]
struct Ready {
    ready: bool,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Serialize)]
struct Request<'a> {
    queries: &'a [MoveQuery],
}

#[derive(Deserialize)]
struct Reply {
    #[serde(default)]
    odds: Option<Vec<Vec<f32>>>,
    #[serde(default)]
    error: Option<String>,
}

struct Pipe {
    _child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
}

/// The sidecar process. Queries are answered one batch at a time.
pub struct MaiaSidecar {
    pipe: Mutex<Pipe>,
    name: String,
}

impl MaiaSidecar {
    /// Starts `command` (a program and its arguments, split on whitespace) and
    /// waits for it to report that the model is loaded.
    pub async fn start(command: &str) -> Result<Self, HumanModelError> {
        let mut parts = command.split_whitespace();
        let program = parts
            .next()
            .ok_or_else(|| HumanModelError::Spawn("empty command".into()))?;
        let mut child = Command::new(program)
            .args(parts)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| HumanModelError::Spawn(format!("{command}: {e}")))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| HumanModelError::Spawn("no stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| HumanModelError::Spawn("no stdout".into()))?;
        let mut pipe = Pipe {
            _child: child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
        };
        let line = timeout(START_TIMEOUT, pipe.stdout.next_line())
            .await
            .map_err(|_| HumanModelError::Spawn("no ready line in time".into()))?
            .map_err(|e| HumanModelError::Spawn(e.to_string()))?
            .ok_or_else(|| HumanModelError::Spawn("the process exited".into()))?;
        let ready: Ready = serde_json::from_str(&line)
            .map_err(|e| HumanModelError::Spawn(format!("bad ready line {line:?}: {e}")))?;
        if !ready.ready {
            return Err(HumanModelError::Spawn(format!("not ready: {line}")));
        }
        Ok(Self {
            pipe: Mutex::new(pipe),
            name: ready.name.unwrap_or_else(|| "Maia-2".to_owned()),
        })
    }
}

#[async_trait]
impl HumanModel for MaiaSidecar {
    async fn odds(&self, queries: &[MoveQuery]) -> Result<Vec<Vec<f32>>, HumanModelError> {
        let failed = |e: String| HumanModelError::Failed(e);
        let mut line =
            serde_json::to_string(&Request { queries }).map_err(|e| failed(e.to_string()))?;
        line.push('\n');
        let mut pipe = self.pipe.lock().await;
        pipe.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| failed(e.to_string()))?;
        pipe.stdin
            .flush()
            .await
            .map_err(|e| failed(e.to_string()))?;
        let answer = timeout(BATCH_TIMEOUT, pipe.stdout.next_line())
            .await
            .map_err(|_| failed("no answer in time".into()))?
            .map_err(|e| failed(e.to_string()))?
            .ok_or_else(|| failed("the process exited".into()))?;
        let reply: Reply = serde_json::from_str(&answer).map_err(|e| failed(e.to_string()))?;
        match (reply.odds, reply.error) {
            (Some(odds), _) if odds.len() == queries.len() => Ok(odds),
            (Some(odds), _) => Err(failed(format!(
                "{} answers for {} queries",
                odds.len(),
                queries.len()
            ))),
            (None, Some(e)) => Err(failed(e)),
            (None, None) => Err(failed("empty answer".into())),
        }
    }

    fn name(&self) -> String {
        self.name.clone()
    }
}
