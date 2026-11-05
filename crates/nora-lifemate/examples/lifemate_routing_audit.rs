//! Offline candidate-recall audit transport. No model, SDK or labels are read.
use nora_lifemate::{
    MAX_JSON_BYTES, Result,
    protocol::{HistoryTurn, ToolSpec},
    routing::route_tools,
};
use serde::Deserialize;
use serde_json::json;
use std::io::{self, BufRead, Read, Write};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    id: String,
    latest_request: String,
    history: Vec<HistoryTurn>,
    tools: Vec<ToolSpec>,
}
fn main() -> Result<()> {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let mut line = Vec::new();
    let mut count = 0;
    loop {
        line.clear();
        if input
            .by_ref()
            .take((MAX_JSON_BYTES + 1) as u64)
            .read_until(b'\n', &mut line)?
            == 0
        {
            break;
        }
        if line.len() > MAX_JSON_BYTES || count >= 2000 {
            return Err("routing audit input exceeds bound".into());
        }
        let request: Request = serde_json::from_slice(&line)?;
        if request.tools.len() > 256 || request.history.len() > 64 {
            return Err("routing audit catalog/history exceeds bound".into());
        }
        let report = route_tools(&request.latest_request, &request.history, &request.tools);
        let selected: Vec<_> = report.tools.iter().map(|t| &t.name).collect();
        serde_json::to_writer(
            &mut output,
            &json!({"id":request.id,"selected_tools":selected,"routing":report}),
        )?;
        output.write_all(b"\n")?;
        count += 1;
    }
    output.flush()?;
    Ok(())
}
