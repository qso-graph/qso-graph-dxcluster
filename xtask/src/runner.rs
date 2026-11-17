// SPDX-License-Identifier: GPL-3.0-or-later
use qso_graph_dxcluster::{model::unbase64, *};
use serde_json::{Value, json};
fn config(v: &Value) -> Result<Config, String> {
    let c = &v["config"];
    let mut cfg = Config::new(
        c["host"].as_str().unwrap_or("example.invalid").into(),
        c["port"].as_u64().unwrap_or(7300) as u16,
        c["identity"].as_str().unwrap_or("Q1ABC").into(),
        Profile::parse(v["profile"].as_str().ok_or("profile")?).ok_or("profile")?,
    );
    cfg.line_limit = c["line_limit"].as_u64().unwrap_or(4096) as usize;
    cfg.retain_third_party_data = c["retain_third_party_data"].as_bool().unwrap_or(false);
    cfg.connect_ms = c["connect_ms"].as_u64().unwrap_or(30000);
    cfg.login_ms = c["login_ms"].as_u64().unwrap_or(30000);
    cfg.identity_ms = c["identity_ms"].as_u64().unwrap_or(30000);
    cfg.setup_ms = c["setup_ms"].as_u64().unwrap_or(30000);
    cfg.silence_ms = c["silence_ms"].as_u64();
    cfg.custom_prompt = c["custom_prompt"].as_str().map(str::to_owned);
    cfg.supported_setup_errors = c["supported_setup_errors"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|s| {
                    s.as_str()
                        .map(str::to_owned)
                        .ok_or("setup-error recognizer")
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    cfg.validate().map_err(str::to_owned)?;
    Ok(cfg)
}
fn json_event(e: Event) -> Value {
    serde_json::from_str(&e.json()).expect("event JSON")
}
fn trace(b: Batch, step: usize, at: u64, events: &mut Vec<Value>, actions: &mut Vec<Value>) {
    for e in b.events {
        events.push(json!({"step":step,"elapsed_ms":at,"event":json_event(e)}));
    }
    for a in b.actions {
        actions.push(json!({"step":step,"elapsed_ms":at,"action":serde_json::from_str::<Value>(&a.json()).expect("action JSON")}));
    }
}
fn pump(
    s: &mut Session,
    at: u64,
    equal: bool,
    step: usize,
    events: &mut Vec<Value>,
    actions: &mut Vec<Value>,
) -> Result<(), String> {
    while let Some((d, t)) = s.next_timer() {
        if d > at || (d == at && !equal && t != Timer::StableReady) {
            break;
        }
        let b = s.fire(t, d).map_err(str::to_owned)?;
        trace(b, step, d, events, actions);
    }
    Ok(())
}
type Trace = (Vec<Value>, Vec<Value>, Option<String>);
pub fn execute(v: &Value) -> Result<Trace, String> {
    if !["Event parser", "Session engine", "Application"]
        .iter()
        .any(|c| v["class"] == *c)
    {
        return Err("invalid conformance class".into());
    }
    let cfg = config(v)?;
    let session = v["class"] != "Event parser";
    let tokens = v["tokens"]
        .as_array()
        .ok_or("tokens")?
        .iter()
        .map(|v| v.as_str().map(str::to_owned).ok_or("token"))
        .collect::<Result<Vec<_>, _>>()?;
    let jitter = v["jitter_ms"]
        .as_array()
        .ok_or("jitter")?
        .iter()
        .map(|v| v.as_u64().ok_or("jitter"))
        .collect::<Result<Vec<_>, _>>()?;
    let mut ctx = Context::ready(cfg.profile);
    ctx.identity = cfg.call.clone();
    ctx.retain_third_party_data = cfg.retain_third_party_data;
    ctx.custom_prompt = cfg.custom_prompt.clone();
    let mut parser = Parser::new(ctx, cfg.line_limit).map_err(str::to_owned)?;
    let mut engine = Session::new(cfg, tokens, jitter).map_err(str::to_owned)?;
    let mut events = vec![];
    let mut actions = vec![];
    let mut last = 0;
    let steps = v["steps"].as_array().ok_or("steps")?;
    for (i, s) in steps.iter().enumerate() {
        let at = s["elapsed_ms"]
            .as_u64()
            .filter(|at| *at >= last && *at <= qso_graph_dxcluster::values::MAX_INTEGER)
            .ok_or("step time")?;
        last = at;
        let kind = s["type"].as_str().ok_or("step type")?;
        let mut permitted = vec!["type", "elapsed_ms"];
        match kind {
            "start" | "restart" | "cancel" | "tick" => {}
            "input" => {
                permitted.extend(["b64", "received_at"]);
                if session {
                    permitted.push("attempt");
                }
                if let Some(receipt) = s.get("received_at")
                    && !receipt
                        .as_str()
                        .is_some_and(qso_graph_dxcluster::values::receipt)
                {
                    return Err("invalid receipt timestamp".into());
                }
            }
            "transport" => {
                permitted.extend(["outcome", "detail"]);
                if session {
                    permitted.push("attempt");
                }
                if s.get("detail").is_some_and(|s| !s.is_string()) {
                    return Err("invalid local detail".into());
                }
            }
            _ => return Err("invalid stimulus type".into()),
        }
        if !s
            .as_object()
            .ok_or("stimulus object")?
            .keys()
            .all(|k| permitted.contains(&k.as_str()))
        {
            return Err("unexpected stimulus member".into());
        }
        if kind == "tick" && i + 1 < steps.len() && steps[i + 1]["elapsed_ms"] == s["elapsed_ms"] {
            return Err("tick must end its equal-time group".into());
        }

        if (i + 1 == steps.len() || steps[i + 1]["elapsed_ms"] != s["elapsed_ms"]) && kind != "tick"
        {
            return Err("equal-time group must end with tick".into());
        }
        if session {
            pump(&mut engine, at, false, i, &mut events, &mut actions)?;
            let b = match kind {
                "start" => engine.start(at),
                "restart" => engine.restart(at),
                "cancel" => engine.cancel(at),
                "tick" => {
                    pump(&mut engine, at, true, i, &mut events, &mut actions)?;
                    continue;
                }
                "transport" => {
                    let attempt = s["attempt"].as_u64().ok_or("attempt")?;
                    match s["outcome"].as_str().ok_or("outcome")? {
                        "connected" => engine.connected(at, attempt),
                        "eof" | "closed" => engine.transport_end(at, attempt, "eof"),
                        "error" => engine.transport_end(at, attempt, "transport_error"),
                        "unsupported_transport" => {
                            engine.transport_end(at, attempt, "unsupported_transport")
                        }
                        "resource_limit" => engine.transport_end(at, attempt, "resource_limit"),
                        _ => return Err("invalid outcome".into()),
                    }
                }
                "input" => {
                    for byte in
                        unbase64(s["b64"].as_str().ok_or("input")?).map_err(str::to_owned)?
                    {
                        let b = engine
                            .input(
                                at,
                                s["attempt"].as_u64().ok_or("attempt")?,
                                byte,
                                s["received_at"].as_str(),
                            )
                            .map_err(str::to_owned)?;
                        trace(b, i, at, &mut events, &mut actions);
                    }
                    continue;
                }
                _ => return Err("invalid step".into()),
            }
            .map_err(str::to_owned)?;
            trace(b, i, at, &mut events, &mut actions);
        } else {
            let batch = match kind {
                "input" => {
                    let mut e = vec![];
                    for byte in
                        unbase64(s["b64"].as_str().ok_or("input")?).map_err(str::to_owned)?
                    {
                        e.extend(
                            parser
                                .push(byte, s["received_at"].as_str())
                                .map_err(str::to_owned)?,
                        );
                    }
                    e
                }
                "transport" if s["outcome"] == "eof" => parser.eof(None).0,
                "tick" => vec![],
                _ => return Err("line vector stimulus".into()),
            };
            for e in batch {
                events.push(json!({"step":i,"elapsed_ms":at,"event":json_event(e)}));
            }
        }
    }
    Ok((
        events,
        actions,
        session.then(|| engine.state.name().to_owned()),
    ))
}
fn without_steps(v: &[Value]) -> Vec<Value> {
    v.iter()
        .map(|v| {
            let mut v = v.clone();
            v.as_object_mut().unwrap().remove("step");
            v
        })
        .collect()
}
pub fn check(v: &Value) -> Result<(), String> {
    if let Some(error) = v["expected_config_error"].as_str() {
        if v["steps"] != json!([])
            || v["expected_events"] != json!([])
            || v["expected_actions"] != json!([])
            || v.get("expected_state").is_some()
        {
            return Err("invalid configuration-rejection vector".into());
        }
        return match config(v) {
            Err(actual) if actual == error => Ok(()),
            Err(actual) => Err(format!(
                "configuration error {actual:?}, expected {error:?}"
            )),
            Ok(_) => Err("invalid configuration admitted".into()),
        };
    }

    let (e, a, state) = execute(v)?;
    if e.as_slice()
        != v["expected_events"]
            .as_array()
            .ok_or("expected_events")?
            .as_slice()
    {
        let expected = v["expected_events"].as_array().ok_or("expected_events")?;
        let first = e
            .iter()
            .zip(expected)
            .position(|(a, b)| a != b)
            .unwrap_or(e.len().min(expected.len()));
        return Err(format!(
            "event {first}: actual {} expected {} (counts {} / {})",
            e.get(first).unwrap_or(&Value::Null),
            expected.get(first).unwrap_or(&Value::Null),
            e.len(),
            expected.len()
        ));
    }
    if a.as_slice()
        != v["expected_actions"]
            .as_array()
            .ok_or("expected_actions")?
            .as_slice()
    {
        return Err(format!(
            "action trace differs: actual {} expected {}",
            json!(a),
            v["expected_actions"]
        ));
    }
    if state.as_deref() != v["expected_state"].as_str() {
        return Err("final state differs".into());
    }
    if v["class"] == "Application" {
        application_roundtrip(v, &a, &e)?;
    }
    let mut split = v.clone();
    let mut steps = vec![];
    for step in v["steps"].as_array().ok_or("steps")? {
        if step["type"] == "input" {
            let bytes = unbase64(step["b64"].as_str().ok_or("bytes")?).map_err(str::to_owned)?;
            for byte in bytes {
                let mut s = step.clone();
                s["b64"] = json!(crate::oracle::b64(&[byte]));
                steps.push(s);
            }
        } else {
            steps.push(step.clone());
        }
    }
    split["steps"] = json!(steps);
    let (se, sa, ss) = execute(&split)?;
    if without_steps(&e) != without_steps(&se)
        || without_steps(&a) != without_steps(&sa)
        || state != ss
    {
        return Err("chunk-split invariance".into());
    }
    Ok(())
}
pub fn all() -> Result<(), String> {
    let mut paths: Vec<_> = std::fs::read_dir("vectors")
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|p| p.path())
        .filter(|p| p.extension().is_some_and(|s| s == "ndjson"))
        .collect();
    paths.sort();
    let mut ids = std::collections::BTreeSet::new();
    for path in paths {
        for (i, line) in std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())?
            .lines()
            .enumerate()
        {
            let v: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
            let id = v["id"].as_str().ok_or("vector id")?;
            if !ids.insert(id.to_owned()) {
                return Err("duplicate vector id".into());
            }
            if v["spec"] != "0.1.0" {
                return Err("vector version".into());
            }
            check(&v).map_err(|e| format!("{}:{} {id}: {e}", path.display(), i + 1))?;
        }
    }
    if ids.is_empty() {
        return Err("no vectors".into());
    }
    println!(
        "{} vectors: complete event/action/state traces and byte-split invariance",
        ids.len()
    );
    Ok(())
}

fn application_roundtrip(v: &Value, actions: &[Value], expected: &[Value]) -> Result<(), String> {
    use qso_graph_dxcluster_cli::recording::{Recorder, Records, Replay, replay_config};
    let mut recorder = Recorder::new(Vec::<u8>::new(), true).map_err(|e| e.to_string())?;
    recorder
        .note(
            0.0,
            &format!(
                "qg-dxc configuration: {}",
                json!({"call":v["config"]["identity"],"profile":v["profile"]})
            ),
        )
        .map_err(|e| e.to_string())?;
    let mut open = false;
    for (index, step) in v["steps"].as_array().unwrap().iter().enumerate() {
        let t = step["elapsed_ms"].as_u64().unwrap() as f64 / 1000.0;
        if step["type"] == "transport" && step["outcome"] == "connected" {
            recorder.open(t).map_err(|e| e.to_string())?;
            open = true;
        }
        if step["type"] == "input" && open {
            let b = crate::oracle::unb64(step["b64"].as_str().unwrap())?;
            recorder.wire(t, "in", &b).map_err(|e| e.to_string())?;
        }
        for wrapped in actions.iter().filter(|a| a["step"] == index) {
            let a = &wrapped["action"];
            if a["type"] == "output" {
                let b = crate::oracle::unb64(a["b64"].as_str().unwrap())?;
                recorder.wire(t, "out", &b).map_err(|e| e.to_string())?;
            }
            if a["type"] == "close" && open {
                recorder.close(t, "cancelled").map_err(|e| e.to_string())?;
                open = false;
            }
        }
        if step["type"] == "transport"
            && ["eof", "error"].iter().any(|s| step["outcome"] == *s)
            && open
        {
            recorder
                .close(
                    t,
                    if step["outcome"] == "eof" {
                        "eof"
                    } else {
                        "transport_error"
                    },
                )
                .map_err(|e| e.to_string())?;
            open = false;
        }
    }
    if open {
        return Err("application vector must close every recorded connection".into());
    }
    let bytes = recorder.into_inner();
    let (call, profile, tokens) = replay_config(std::io::Cursor::new(&bytes))?;
    let mut replay = Replay::new(call, profile, tokens)?;
    let mut records = Records::new(std::io::Cursor::new(&bytes));
    let mut actual = vec![];
    while let Some(record) = records.next_record()? {
        actual.extend(replay.record(&record)?);
    }
    actual.extend(replay.finish()?);
    let actual: Vec<_> = actual
        .into_iter()
        .filter(|e| e.kind != "connection_state")
        .map(json_event)
        .collect();
    let wanted: Vec<_> = expected
        .iter()
        .filter(|w| w["event"]["kind"] != "connection_state")
        .map(|w| w["event"].clone())
        .collect();
    if actual != wanted {
        return Err(
            "application recording/replay content trace differs from independent oracle".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn independent_oracles_reject_mutated_expectations() {
        // QGDXC-VEC-0005 QGDXC-VEC-0006
        let mut v = crate::generate::vector(
            "oracle-mutation",
            "arcluster",
            b"DX de Q1ABC: 1 Q2ABC 0000Z",
            "lf",
            None,
        );
        super::check(&v).unwrap();
        v["expected_events"][0]["event"]["fields"]["frequency_hz"] = serde_json::json!(1);
        assert!(super::check(&v).is_err());
    }
}
