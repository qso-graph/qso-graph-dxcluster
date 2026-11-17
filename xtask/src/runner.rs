// SPDX-License-Identifier: GPL-3.0-or-later
use qso_graph_dxcluster::{model::unbase64, *};
use serde_json::{Value, json};
fn config(v: &Value) -> Result<Config, String> {
    let c = v["config"].as_object().ok_or("configuration object")?;
    let profile = Profile::parse(v["profile"].as_str().ok_or("profile")?).ok_or("profile")?;
    let mut cfg = Config::new("example.invalid".into(), 7300, "Q1ABC".into(), profile);
    for (key, value) in c {
        let invalid = || format!("invalid configuration member: {key}");
        let text = || value.as_str().ok_or_else(invalid);
        let integer = || value.as_u64().ok_or_else(invalid);
        let size = || usize::try_from(integer()?).map_err(|_| invalid());
        match key.as_str() {
            "host" => cfg.host = text()?.into(),
            "port" => cfg.port = u16::try_from(integer()?).map_err(|_| invalid())?,
            "identity" => cfg.call = text()?.into(),
            "profile" => {
                if Profile::parse(text()?) != Some(profile) {
                    return Err("configuration profile differs from vector profile".into());
                }
            }
            "line_limit" => cfg.line_limit = size()?,
            "retain_third_party_data" => {
                cfg.retain_third_party_data = value.as_bool().ok_or_else(invalid)?
            }
            "connect_ms" => cfg.connect_ms = integer()?,
            "login_ms" => cfg.login_ms = integer()?,
            "identity_ms" => cfg.identity_ms = integer()?,
            "setup_ms" => cfg.setup_ms = integer()?,
            "silence_ms" => {
                cfg.silence_ms = if value.is_null() {
                    None
                } else {
                    Some(integer()?)
                }
            }
            "spacing_ms" => cfg.spacing_ms = integer()?,
            "window_ms" => cfg.window_ms = integer()?,
            "window_lines" => cfg.window_lines = size()?,
            "custom_prompt" => {
                cfg.custom_prompt = if value.is_null() {
                    None
                } else {
                    Some(text()?.into())
                }
            }
            "supported_setup_errors" => {
                cfg.supported_setup_errors = value
                    .as_array()
                    .ok_or_else(invalid)?
                    .iter()
                    .map(|v| v.as_str().map(str::to_owned).ok_or_else(invalid))
                    .collect::<Result<Vec<_>, _>>()?
            }
            _ => return Err(format!("unsupported configuration member: {key}")),
        }
    }
    cfg.validate().map_err(str::to_owned)?;
    Ok(cfg)
}
fn json_event(e: Event) -> Value {
    serde_json::from_str(&e.json()).expect("event JSON")
}
struct Inputs {
    tokens: std::vec::IntoIter<String>,
    jitter: std::vec::IntoIter<u64>,
}
fn trace(
    engine: &mut Session,
    inputs: &mut Inputs,
    b: Batch,
    step: usize,
    at: u64,
    events: &mut Vec<Value>,
    actions: &mut Vec<Value>,
) -> Result<(), String> {
    for e in b.events {
        events.push(json!({"step":step,"elapsed_ms":at,"event":json_event(e)}));
    }
    for a in b.actions {
        let supplied = match a {
            Action::ReadinessToken { attempt } => engine.readiness_token(at, attempt,
                inputs.tokens.next().ok_or("missing readiness token for connection attempt")?),
            Action::RetryJitter { .. } => engine.retry_jitter(at,
                inputs.jitter.next().ok_or("missing retry jitter")?),
            a => {
                actions.push(json!({"step":step,"elapsed_ms":at,"action":serde_json::from_str::<Value>(&a.json()).expect("action JSON")}));
                continue;
            }
        }.map_err(str::to_owned)?;
        trace(engine, inputs, supplied, step, at, events, actions)?;
    }
    Ok(())
}

fn pump(
    s: &mut Session,
    inputs: &mut Inputs,
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
        trace(s, inputs, b, step, d, events, actions)?;
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
    let tokens: Vec<String> = v["tokens"]
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
    let mut unique = tokens.clone();
    unique.sort();
    unique.dedup();
    if unique.len() != tokens.len()
        || tokens
            .iter()
            .any(|t| !qso_graph_dxcluster::values::token(t))
    {
        return Err("invalid or repeated readiness tokens".into());
    }
    let mut inputs = Inputs {
        tokens: tokens.into_iter(),
        jitter: jitter.into_iter(),
    };
    let mut engine = Session::new(cfg).map_err(str::to_owned)?;
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
            pump(
                &mut engine,
                &mut inputs,
                at,
                false,
                i,
                &mut events,
                &mut actions,
            )?;
            let b = match kind {
                "start" => engine.start(at),
                "restart" => engine.restart(at),
                "cancel" => engine.cancel(at),
                "tick" => {
                    pump(
                        &mut engine,
                        &mut inputs,
                        at,
                        true,
                        i,
                        &mut events,
                        &mut actions,
                    )?;
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
                        trace(
                            &mut engine,
                            &mut inputs,
                            b,
                            i,
                            at,
                            &mut events,
                            &mut actions,
                        )?;
                    }
                    continue;
                }
                _ => return Err("invalid step".into()),
            }
            .map_err(str::to_owned)?;
            trace(
                &mut engine,
                &mut inputs,
                b,
                i,
                at,
                &mut events,
                &mut actions,
            )?;
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
        application_roundtrip(
            v,
            &a,
            v["expected_events"].as_array().ok_or("expected_events")?,
        )?;
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
        .take(1025)
        .collect();
    if paths.len() > 1024 {
        return Err("vector file limit exceeded".into());
    }
    paths.sort();
    let mut ids = std::collections::BTreeSet::new();
    let mut budget = crate::input::Budget::new();
    for path in paths {
        for (i, line) in crate::input::lines(&path)?.enumerate() {
            let v: Value = serde_json::from_str(&line?).map_err(|e| e.to_string())?;
            let id = v["id"].as_str().ok_or("vector id")?;
            budget.charge(id.len() + 256)?;
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
    use qso_graph_dxcluster_cli::recording::{Recorder, ReplayStream};
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
    let mut replay = ReplayStream::new(std::io::Cursor::new(&bytes), false)?;
    let mut actual = vec![];
    while let Some((_, _, events)) = replay.next_record()? {
        actual.extend(events);
    }
    actual.extend(replay.finish()?);
    let actual: Vec<_> = actual.into_iter().map(json_event).collect();
    let wanted: Vec<_> = expected.iter().map(|w| w["event"].clone()).collect();
    if actual != wanted {
        return Err(
            "application recording/replay event trace differs from independent oracle".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    #[test]
    fn vector_attempt_requests_require_explicit_resources() {
        // QGDXC-VEC-0006 QGDXC-DSP-0004 QGDXC-SES-0010
        let v = json!({
            "class": "Session engine", "profile": "auto", "config": {"identity": "Q1ABC"},
            "tokens": ["QGDXC0000000000000001", "QGDXC0000000000000002"], "jitter_ms": [5],
            "steps": [
                {"type": "start", "elapsed_ms": 0}, {"type": "tick", "elapsed_ms": 0},
                {"type": "transport", "elapsed_ms": 1, "attempt": 1, "outcome": "error"},
                {"type": "tick", "elapsed_ms": 1}, {"type": "tick", "elapsed_ms": 5006}
            ]
        });
        let (_, actions, _) = super::execute(&v).unwrap();
        let connects: Vec<_> = actions
            .iter()
            .filter(|a| a["action"]["type"] == "connect")
            .collect();
        assert_eq!(connects.len(), 2);
        assert_eq!(connects[1]["elapsed_ms"], 5006);
        assert_eq!(connects[1]["action"]["attempt"], 2);
        let mut missing_token = v.clone();
        missing_token["tokens"].as_array_mut().unwrap().pop();
        assert_eq!(
            super::execute(&missing_token).unwrap_err(),
            "missing readiness token for connection attempt"
        );
        let mut missing_jitter = v.clone();
        missing_jitter["jitter_ms"] = json!([]);
        assert_eq!(
            super::execute(&missing_jitter).unwrap_err(),
            "missing retry jitter"
        );
        let mut repeated = v.clone();
        repeated["tokens"][1] = repeated["tokens"][0].clone();
        assert!(super::execute(&repeated).is_err());
        let mut excess = v;
        excess["jitter_ms"] = json!([1001]);
        assert!(super::execute(&excess).is_err());
    }
    #[test]
    fn replay_gate_compares_connection_state_and_retry_delays() {
        // QGDXC-REC-0004 QGDXC-SES-0010
        let v = crate::scenarios::vectors()
            .into_iter()
            .find(|v| v["id"] == "application-replay-retry-exhaustion")
            .unwrap();
        let actions = v["expected_actions"].as_array().unwrap();
        let expected = v["expected_events"].as_array().unwrap();
        super::check(&v).unwrap();
        for reason in ["retry_scheduled", "retry_exhausted"] {
            let mut mutated = expected.clone();
            let event = mutated
                .iter_mut()
                .find(|e| e["event"]["fields"]["reason"] == reason)
                .unwrap();
            event["event"]["fields"]["reason"] = json!("cancelled");
            assert!(super::application_roundtrip(&v, actions, &mutated).is_err());
        }
        let mut mutated = expected.clone();
        let event = mutated
            .iter_mut()
            .find(|e| e["event"]["fields"]["retry_in_ms"] == 10000)
            .unwrap();
        event["event"]["fields"]["retry_in_ms"] = json!(5000);
        assert!(super::application_roundtrip(&v, actions, &mutated).is_err());
    }
    #[test]
    fn stricter_pacing_vectors_apply_configuration() {
        // QGDXC-SES-0011 QGDXC-VEC-0006
        for v in crate::scenarios::vectors()
            .iter()
            .filter(|v| v["id"].as_str().unwrap().starts_with("stricter-"))
        {
            super::check(v).unwrap();
        }
    }
    #[test]
    fn vector_configuration_rejects_unknown_or_invalid_members() {
        // QGDXC-VEC-0006
        let base = crate::generate::vector("config-validation", "rbn", b"text", "lf", None);
        for (key, value) in [
            ("unknown_setting", json!(true)),
            ("spacing_ms", json!("2000")),
            ("window_ms", json!(null)),
            ("window_lines", json!(-1)),
            ("port", json!(65537)),
            ("line_limit", json!("4096")),
            ("host", json!(false)),
            ("identity", json!(0)),
            ("retain_third_party_data", json!("false")),
            ("login_ms", json!(null)),
            ("silence_ms", json!(false)),
            ("custom_prompt", json!(0)),
            ("supported_setup_errors", json!("error")),
            ("profile", json!("arcluster")),
        ] {
            let mut v = base.clone();
            v["config"][key] = value;
            assert!(super::config(&v).is_err(), "admitted {key}");
        }
    }
    #[test]
    fn pacing_mutation_invalidates_old_expectations() {
        // QGDXC-SES-0011 QGDXC-VEC-0006
        let mut v = crate::scenarios::vectors()
            .into_iter()
            .find(|v| v["id"] == "startup-cccluster-cccluster")
            .unwrap();
        super::check(&v).unwrap();
        v["config"]["spacing_ms"] = json!(2000);
        assert!(super::check(&v).is_err());
    }
    #[test]
    fn overlong_banner_cannot_establish_session_identity() {
        // QGDXC-EVT-0010 QGDXC-ARC-0001 QGDXC-SES-0004
        let v = crate::scenarios::vectors()
            .into_iter()
            .find(|v| v["id"] == "overlong-ar-banner-no-profile-evidence")
            .unwrap();
        super::check(&v).unwrap();
    }
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
