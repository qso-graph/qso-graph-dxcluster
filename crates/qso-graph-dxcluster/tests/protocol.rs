// SPDX-License-Identifier: GPL-3.0-or-later
use qso_graph_dxcluster::{
    framing::{Framer, Telnet},
    model::{base64, escape, unbase64},
    values, *,
};
fn parse(profile: Profile, bytes: &[u8], retain: bool) -> Vec<Event> {
    let mut c = Context::ready(profile);
    c.retain_third_party_data = retain;
    let mut p = Parser::new(c, 256).unwrap();
    let mut events = Vec::new();
    for b in bytes {
        events.extend(p.push(*b, None).unwrap());
    }
    events.extend(p.eof(None).0);
    events
}
#[test]
fn exact_values_and_identity() {
    // QGDXC-IO-0003 QGDXC-IO-0006 QGDXC-DX-0002 QGDXC-EVT-0005 QGDXC-EVT-0009
    for good in ["Q1ABC", "q1abc/P-99", "Q1ABC-1"] {
        assert!(values::login(good));
    }
    for bad in [
        "Q1ABC-0",
        "Q1ABC-00",
        "Q1ABC-01",
        "Q1ABC\r\nquit",
        "QABC",
        "123",
        "Q1ABC-x",
    ] {
        assert!(!values::login(bad));
        let cfg = Config::new("example.invalid".into(), 7300, bad.into(), Profile::Auto);
        assert!(Session::new(cfg, vec![], vec![]).is_err());
    }
    for (s, n) in [
        ("0.001", 1),
        ("14025.123", 14_025_123),
        ("999999999999.999", 999_999_999_999_999),
    ] {
        assert_eq!(values::frequency(s), Some(n));
    }
    for s in [
        "0",
        "1.",
        "-1",
        "+1",
        "1e3",
        "1,000",
        ".1",
        "1.0001",
        "1000000000000",
    ] {
        assert_eq!(values::frequency(s), None);
    }
    assert_eq!(values::date("29-fEb-2000"), Some("2000-02-29".into()));
    assert!(values::date("29-Feb-1900").is_none());
    assert!(values::date("1-Jan-0000").is_none());
    assert!(values::grid("aR09"));
    assert!(!values::grid("SS99"));
    assert!(!values::grid("AA00aa"));
}
#[test]
fn delimiters_bounds_and_encoding() {
    // QGDXC-FRM-0001 QGDXC-FRM-0002 QGDXC-FRM-0003 QGDXC-FRM-0005 QGDXC-FRM-0006 QGDXC-FRM-0007
    let events = parse(Profile::ArCluster, b"\r\n \n\xfe\rabc\rpartial", false);
    assert_eq!(
        events.iter().map(|e| e.raw_ending).collect::<Vec<_>>(),
        ["crlf", "lf", "cr", "cr", "none"]
    );
    assert_eq!(events[2].encoding, "latin1");
    assert_eq!(events[2].get("text"), Some("þ"));
    assert_eq!(events[4].get("reason"), Some("partial"));
    let mut data = vec![b'x'; 257];
    data.extend_from_slice(b"DX de Q1ABC: 1 Q2XYZ 0000Z\r\nDX de Q1ABC: 1 Q2XYZ 0000Z\n");
    let e = parse(Profile::ArCluster, &data, false);
    assert_eq!(e.len(), 2);
    assert_eq!(e[0].get("reason"), Some("line_too_long"));
    assert_eq!(e[0].raw_line.len(), 256);
    assert_eq!(e[0].raw_status, "truncated");
    assert_eq!(e[1].kind, "spot");
    assert!(Framer::new(255).is_err());
    assert!(Framer::new(65537).is_err());
}
#[test]
fn common_spots_and_dispatch() {
    // QGDXC-DX-0001 QGDXC-DX-0003 QGDXC-DX-0004 QGDXC-DX-0005 QGDXC-DX-0006
    // QGDXC-EVT-0001 QGDXC-EVT-0003 QGDXC-EVT-0004 QGDXC-EVT-0006 QGDXC-EVT-0010
    // QGDXC-ARC-0003 QGDXC-CCC-0003 QGDXC-DSP-0005 QGDXC-DSP-0007 QGDXC-RBN-0002
    let e = parse(
        Profile::DxSpider,
        b"DX de Q9XYZ-2-#: 14025.0 Q2ABC/P CW  12 dB 1832Z   7  \x07\x07\n",
        false,
    )
    .remove(0);
    assert_eq!(e.kind, "spot");
    assert_eq!(e.get("comment"), Some("CW  12 dB"));
    assert_eq!(e.get("time_of_day"), Some("18:32"));
    assert_eq!(e.get("spot_kind"), Some("skimmer"));
    assert_eq!(e.get("source"), Some("cluster"));
    assert!(!e.fields.contains_key("date"));
    assert!(!e.fields.contains_key("mode"));
    assert!(e.json().contains("   7  "));
    for token in ["!", "_", "123", "a", "q1abc", "Q9XYZ-2-#"] {
        let bytes = format!("DX de {token}: 1 {token} 0000Z\n");
        assert_eq!(
            parse(Profile::ArCluster, bytes.as_bytes(), false)[0].kind,
            "spot"
        );
    }
    for line in [
        "DX de Q1ABC: 1 Q2ABC 2460Z",
        "x DX de Q1ABC: 1 Q2ABC 0000Z",
        "DX de Q1ABC: 1 Q2ABC 0000Z BAD",
        "DX de Q1ABC: -1 Q2ABC 0000Z",
    ] {
        assert_eq!(
            parse(Profile::ArCluster, format!("{line}\n").as_bytes(), false)[0].kind,
            "unrecognized_line"
        );
    }
    let e = parse(Profile::Auto, b"DX de Q1ABC: 1 Q2ABC 0000Z\nCC11^\n", false);
    assert_eq!(e[0].get("reason"), Some("before_identification"));
}
#[test]
fn enhanced_and_rbn() {
    // QGDXC-DSP-0006 QGDXC-CCC-0004 QGDXC-EVT-0007 QGDXC-RBN-0003 QGDXC-RBN-0004 QGDXC-RBN-0005
    let line = b"CC11^14025^Q2ABC^1-Nov-2026^1832Z^A^B^Q1ABC^1^2^Q3ABC^4^5^6^7^^^QX^QY^aA00^\n";
    let e = parse(Profile::DxSpider, line, false).remove(0);
    assert_eq!(e.kind, "spot");
    assert_eq!(e.get("comment"), Some("A^B"));
    assert_eq!(e.get("grid"), Some("aA00"));
    assert_eq!(
        parse(Profile::CcCluster, line, false)[0].get("reason"),
        Some("privacy_withheld")
    );
    for (comment, mode, slot) in [
        ("CW -12 dB 23 WPM NCDXF B", Some("CW"), None),
        ("RTTY +3 dB 45 BPS DX", Some("RTTY"), None),
        ("FT8 -2 dB AA00 CQ", Some("FT8"), Some("AA00")),
        ("FT4 12 dB 1844 CQ", Some("FT4"), None),
        ("FT8 0 dB CQ", Some("FT8"), None),
        ("FT8 32768 dB CQ", None, None),
        ("CW 1 dB -1 WPM CQ", None, None),
        ("CW 1 dB 20 BPS CQ", None, None),
    ] {
        let e = parse(
            Profile::Rbn,
            format!("DX de Q1ABC-#: 14074 Q2ABC {comment} 1832Z\n").as_bytes(),
            false,
        )
        .remove(0);
        assert_eq!(e.kind, "spot");
        assert_eq!(e.get("mode"), mode, "{comment}");
        assert_eq!(e.get("grid"), slot);
    }
}
#[test]
fn nonspot_layouts() {
    // QGDXC-DSP-0008 QGDXC-CCC-0005 QGDXC-ARC-0004 QGDXC-ARC-0005 QGDXC-EVT-0008
    for (s, kind) in [
        ("To ALL de Q1ABC: Hello", "announcement"),
        ("WX de Q1ABC: Rain", "announcement"),
        ("Q2ABC de Q1ABC: Hi", "talk"),
        ("WWV de Q1ABC <23>: SFI=100, A=x, K=3, quiet", "wwv"),
        (
            "WCY de Q1ABC <0> : K=3 expK=4 A=10 R=5 SFI=99 SA=quiet GMF=quiet Au=no",
            "wcy",
        ),
    ] {
        let e = parse(Profile::DxSpider, format!("{s}\n").as_bytes(), false).remove(0);
        assert_eq!(e.kind, kind, "{s}");
    }
    for s in [
        "DX de broken: text",
        "To ALL de : text",
        "WWV de Q1ABC <24>: bad",
        "WX de : bad",
    ] {
        assert_eq!(
            parse(Profile::DxSpider, format!("{s}\n").as_bytes(), false)[0].get("reason"),
            Some("malformed")
        );
    }
    assert_eq!(
        parse(
            Profile::CcCluster,
            b"To ALL de Q1ABC <Q2ABC> : 1-Nov text\n",
            false
        )[0]
        .get("via_call"),
        Some("Q2ABC")
    );
    assert_eq!(
        parse(
            Profile::ArCluster,
            b"14025 Q1ABC 1-Nov-2026 1832Z text <Q2ABC>\n",
            false
        )[0]
        .get("reason"),
        Some("unsupported_layout")
    );
    for p in [Profile::ArCluster, Profile::Rbn] {
        assert_eq!(
            parse(p, b"CC11^x\n", true)[0].get("reason"),
            Some("unsupported_layout")
        );
    }
}
#[test]
fn privacy_and_terminal() {
    // QGDXC-SEC-0003 QGDXC-SEC-0005
    // Documentation addresses are deliberately assembled, also used by the sanitizer's planted checks.
    let ip = [192, 0, 2, 1]
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(".");
    for k in [0, 1] {
        let s = if k == 0 {
            format!("DX de {ip}: 1 Q2ABC 0000Z\n")
        } else {
            format!("DX de Q1ABC: 1 {ip} 0000Z\n")
        };
        let e = parse(Profile::DxSpider, s.as_bytes(), false).remove(0);
        assert_eq!(e.raw_status, "withheld");
        assert!(e.raw_line.is_empty());
        assert_eq!(e.get("text"), Some(""));
    }
    for line in [
        format!("Q1ABC de {ip}: hi\n"),
        format!("{ip} de Q1ABC: hi\n"),
    ] {
        assert_eq!(
            parse(Profile::DxSpider, line.as_bytes(), false)[0].raw_status,
            "withheld"
        );
    }
    let s = format!("DX de Q1ABC: 1 Q2ABC {ip} [2001:db8::1] \x1b\x07 0000Z\n");
    let e = parse(Profile::DxSpider, s.as_bytes(), false).remove(0);
    assert_eq!(e.raw_status, "redacted");
    assert!(!e.json().contains(&ip));
    assert!(e.json().contains("<ip>"));
    assert_eq!(
        parse(Profile::DxSpider, s.as_bytes(), true)[0].raw_status,
        "exact"
    );
    assert!(!escape("\x1b\x07\r\n\u{85}").chars().any(char::is_control));
    let mut f = vec![""; 25];
    f[0] = "CC11";
    f[1] = "1";
    f[2] = "Q2ABC";
    f[3] = "1-Nov-2026";
    f[4] = "1832";
    f[6] = "Q1ABC";
    f[22] = "12.34/-56.78";
    f[23] = &ip;
    let s = format!("{}\n", f.join("^"));
    let e = parse(Profile::CcCluster, s.as_bytes(), false).remove(0);
    assert_eq!(e.kind, "spot");
    assert_eq!(e.raw_status, "redacted");
    assert!(!e.json().contains("12.34"));
    assert!(!e.json().contains(&ip));
    assert!(
        parse(Profile::CcCluster, s.as_bytes(), true)[0]
            .json()
            .contains("12.34")
    );
}
#[test]
fn telnet_all_branches_and_bounds() {
    // QGDXC-FRM-0008 QGDXC-SEC-0001 QGDXC-VEC-0002
    for cmd in [
        vec![255, 255],
        vec![255, 251, 1],
        vec![255, 252, 0],
        vec![255, 253, 255],
        vec![255, 254, 1],
        vec![255, 241],
        vec![255, 250, 1, 255, 255, 240, 255, 240],
    ] {
        for split in 0..=cmd.len() {
            let mut t = Telnet::default();
            let mut out = vec![];
            for b in cmd[..split].iter().chain(&cmd[split..]) {
                if let Some(b) = t.push(*b).unwrap() {
                    out.push(b);
                }
            }
            t.eof().unwrap();
            assert_eq!(out, if cmd == [255, 255] { vec![255] } else { vec![] });
        }
    }
    for len in [4096, 4097] {
        let mut t = Telnet::default();
        let mut b = vec![255, 250];
        b.resize(len - 2, 0);
        b.extend([255, 240]);
        let result = b.into_iter().try_for_each(|b| t.push(b).map(|_| ()));
        assert_eq!(result.is_ok(), len == 4096);
    }
    for cmd in [&[255][..], &[255, 251][..]] {
        let mut t = Telnet::default();
        for b in cmd {
            t.push(*b).unwrap();
        }
        assert!(t.eof().is_ok());
    }
    let mut t = Telnet::default();
    t.push(255).unwrap();
    t.push(250).unwrap();
    assert!(t.eof().is_err());
}
#[test]
fn base64_roundtrip() {
    for len in 0..300 {
        let b: Vec<_> = (0..len).map(|n| n as u8).collect();
        assert_eq!(unbase64(&base64(&b)).unwrap(), b);
    }
    for s in ["A===", "AA=A", "AB==", "AAAA=", "!!!!"] {
        assert!(unbase64(s).is_err());
    }
}
