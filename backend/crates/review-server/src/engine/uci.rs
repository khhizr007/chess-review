//! A single UCI engine process.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use review_core::{Eval, Side, UciScore};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::{Instant, timeout_at};
use tokio_util::sync::CancellationToken;

use super::{AnalysisRequest, EngineError, EngineLine, PositionAnalysis, SearchLimit};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const STOP_GRACE: Duration = Duration::from_secs(3);

pub struct UciEngine {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    name: String,
    multipv: u8,
    chess960: bool,
    /// False once the process is in an unknown state; the pool discards it.
    healthy: bool,
}

impl UciEngine {
    pub async fn spawn(path: &Path, threads: u32, hash_mb: u32) -> Result<Self, EngineError> {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| EngineError::Spawn(format!("{}: {e}", path.display())))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| EngineError::Spawn("no stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| EngineError::Spawn("no stdout".into()))?;
        let mut engine = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            multipv: 1,
            chess960: false,
            healthy: true,
        };
        let deadline = Instant::now() + HANDSHAKE_TIMEOUT;
        engine.send("uci").await?;
        loop {
            let line = engine.read_line_until(deadline).await?;
            if let Some(name) = line.strip_prefix("id name ") {
                engine.name = name.trim().to_owned();
            } else if line.trim() == "uciok" {
                break;
            }
        }
        engine
            .send(&format!("setoption name Threads value {threads}"))
            .await?;
        engine
            .send(&format!("setoption name Hash value {hash_mb}"))
            .await?;
        engine.sync(deadline).await?;
        Ok(engine)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_healthy(&self) -> bool {
        self.healthy
    }

    async fn send(&mut self, command: &str) -> Result<(), EngineError> {
        let result = async {
            self.stdin.write_all(command.as_bytes()).await?;
            self.stdin.write_all(b"\n").await?;
            self.stdin.flush().await
        }
        .await;
        result.map_err(|e| {
            self.healthy = false;
            EngineError::Crashed(e.to_string())
        })
    }

    async fn read_line_until(&mut self, deadline: Instant) -> Result<String, EngineError> {
        match timeout_at(deadline, self.stdout.next_line()).await {
            Ok(Ok(Some(line))) => Ok(line),
            Ok(Ok(None)) => {
                self.healthy = false;
                let status = self.child.try_wait().ok().flatten();
                Err(EngineError::Crashed(format!(
                    "stdout closed (exit status {status:?})"
                )))
            }
            Ok(Err(e)) => {
                self.healthy = false;
                Err(EngineError::Crashed(e.to_string()))
            }
            Err(_) => {
                self.healthy = false;
                Err(EngineError::Timeout)
            }
        }
    }

    async fn sync(&mut self, deadline: Instant) -> Result<(), EngineError> {
        self.send("isready").await?;
        while self.read_line_until(deadline).await?.trim() != "readyok" {}
        Ok(())
    }

    /// Searches one position. On cancellation or timeout the search is stopped
    /// and the engine is left ready for reuse when it responds in time.
    pub async fn analyze(
        &mut self,
        request: &AnalysisRequest,
        cancel: &CancellationToken,
        limit: Duration,
    ) -> Result<PositionAnalysis, EngineError> {
        let side_to_move = side_to_move(&request.fen)?;
        let deadline = Instant::now() + limit;
        if request.multipv != self.multipv {
            self.send(&format!(
                "setoption name MultiPV value {}",
                request.multipv.max(1)
            ))
            .await?;
            self.multipv = request.multipv.max(1);
        }
        if request.chess960 != self.chess960 {
            self.send(&format!(
                "setoption name UCI_Chess960 value {}",
                request.chess960
            ))
            .await?;
            self.chess960 = request.chess960;
        }
        self.sync(deadline).await?;
        self.send(&format!("position fen {}", request.fen)).await?;
        let go = match request.limit {
            SearchLimit::Depth { depth } => format!("go depth {}", depth.max(1)),
            SearchLimit::MoveTime { millis } => format!("go movetime {}", millis.max(10)),
        };
        self.send(&go).await?;

        let mut lines: BTreeMap<u8, EngineLine> = BTreeMap::new();
        loop {
            let line = tokio::select! {
                biased;
                () = cancel.cancelled() => {
                    self.stop_search().await;
                    return Err(EngineError::Cancelled);
                }
                line = self.read_line_until(deadline) => match line {
                    Ok(line) => line,
                    Err(EngineError::Timeout) => {
                        self.healthy = true; // give the stop below a chance
                        self.stop_search().await;
                        return Err(EngineError::Timeout);
                    }
                    Err(e) => return Err(e),
                },
            };
            if line.starts_with("bestmove") {
                break;
            }
            if let Some(info) = parse_info(&line, side_to_move) {
                lines.insert(info.rank, info);
            }
        }
        if lines.is_empty() {
            return Err(EngineError::Protocol(format!(
                "no principal variation for {}",
                request.fen
            )));
        }
        Ok(PositionAnalysis {
            lines: lines.into_values().collect(),
        })
    }

    /// Sends `stop` and waits for `bestmove`; marks the engine unhealthy if it
    /// does not answer.
    async fn stop_search(&mut self) {
        if self.send("stop").await.is_err() {
            return;
        }
        let deadline = Instant::now() + STOP_GRACE;
        loop {
            match self.read_line_until(deadline).await {
                Ok(line) if line.starts_with("bestmove") => return,
                Ok(_) => continue,
                Err(_) => {
                    self.healthy = false;
                    return;
                }
            }
        }
    }

    pub async fn shutdown(mut self) {
        let _ = self.send("quit").await;
        let _ = tokio::time::timeout(Duration::from_secs(1), self.child.wait()).await;
    }
}

fn side_to_move(fen: &str) -> Result<Side, EngineError> {
    match fen.split_whitespace().nth(1) {
        Some("w") => Ok(Side::White),
        Some("b") => Ok(Side::Black),
        _ => Err(EngineError::Protocol(format!("invalid FEN {fen:?}"))),
    }
}

/// Parses a final (non-bound) `info` line with a score and a PV.
pub fn parse_info(line: &str, side_to_move: Side) -> Option<EngineLine> {
    let mut tokens = line.split_whitespace();
    if tokens.next()? != "info" {
        return None;
    }
    let mut depth = None;
    let mut rank = 1u8;
    let mut score = None;
    let mut pv = Vec::new();
    while let Some(token) = tokens.next() {
        match token {
            "depth" => depth = tokens.next()?.parse().ok(),
            "multipv" => rank = tokens.next()?.parse().ok()?,
            "score" => {
                let kind = tokens.next()?;
                let value: i32 = tokens.next()?.parse().ok()?;
                score = Some(match kind {
                    "cp" => UciScore::Cp(value),
                    "mate" => UciScore::Mate(value),
                    _ => return None,
                });
            }
            "lowerbound" | "upperbound" => return None,
            "pv" => {
                pv.extend(tokens.by_ref().map(str::to_owned));
                break;
            }
            // Skip the value of other numeric fields; `string` consumes the rest.
            "string" => return None,
            "seldepth" | "time" | "nodes" | "nps" | "hashfull" | "tbhits" | "cpuload"
            | "currmovenumber" => {
                tokens.next();
            }
            "wdl" => {
                tokens.next();
                tokens.next();
                tokens.next();
            }
            _ => {}
        }
    }
    if pv.is_empty() {
        return None;
    }
    Some(EngineLine {
        rank,
        eval: Eval::from_uci(side_to_move, score?),
        depth: depth?,
        pv,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multipv_info_lines() {
        let line = "info depth 18 seldepth 25 multipv 2 score cp -34 wdl 10 900 90 nodes 123 nps 1000 hashfull 3 tbhits 0 time 50 pv e7e5 g1f3 b8c6";
        let info = parse_info(line, Side::Black).unwrap();
        assert_eq!(info.rank, 2);
        assert_eq!(info.depth, 18);
        assert_eq!(info.eval, Eval::Cp { cp: 34 });
        assert_eq!(info.pv, vec!["e7e5", "g1f3", "b8c6"]);
    }

    #[test]
    fn mate_scores_and_bounds() {
        let mate = parse_info(
            "info depth 5 multipv 1 score mate -2 pv e1e2 d8d2",
            Side::White,
        )
        .unwrap();
        assert_eq!(
            mate.eval,
            Eval::Mate {
                winner: Side::Black,
                moves: 2
            }
        );
        assert!(parse_info("info depth 9 score cp 20 lowerbound pv e2e4", Side::White).is_none());
        assert!(parse_info("info string NNUE evaluation enabled", Side::White).is_none());
        assert!(parse_info("info depth 9 currmove e2e4 currmovenumber 1", Side::White).is_none());
    }
}
