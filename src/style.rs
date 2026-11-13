//! Explicit, local correction memory; no semantic or tone inference.
use crate::{Error, check};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
const MAX_RULES: usize = 64;
const MAX_SAMPLE: usize = 4096;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StyleProfile {
    pub version: u32,
    pub rules: Vec<Correction>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Correction {
    pub before: String,
    pub after: String,
    pub kind: String,
}
fn profile(v: Option<&Value>) -> Result<StyleProfile, Error> {
    let p: StyleProfile = match v {
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|_| Error("文体設定の形式を確認してください。"))?,
        None => StyleProfile {
            version: 1,
            rules: vec![],
        },
    };
    if p.version != 1 || p.rules.len() > MAX_RULES {
        return Err(Error(
            "文体設定はversionを1とし、修正例を64件以内にしてください。",
        ));
    }
    for r in &p.rules {
        validate(&r.before, &r.after)?;
        if !eligible(&r.before)
            || !eligible(&r.after)
            || r.before
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
                != r.after
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect::<String>()
        {
            return Err(Error("学習条件に合わない修正です。"));
        }
        if r.kind != kind(&r.before, &r.after) {
            return Err(Error("修正前後の文章とkindの値が一致していません。"));
        }
    }
    if serde_json::to_vec(&p)
        .map_err(|_| Error("設定を出力できません。"))?
        .len()
        > 65536
    {
        return Err(Error("設定はJSONで64KiBまでです。"));
    }
    Ok(p)
}
fn compact(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}
fn kind(a: &str, b: &str) -> &'static str {
    if compact(a) == compact(b) {
        "layout"
    } else {
        "explicit_wording"
    }
}
fn validate(a: &str, b: &str) -> Result<(), Error> {
    if a.trim().is_empty()
        || b.trim().is_empty()
        || a.len() > MAX_SAMPLE
        || b.len() > MAX_SAMPLE
        || a == b
    {
        return Err(Error(
            "修正前と修正後には、それぞれ空でない異なる文章をUTF-8で4096バイト以内で指定してください。",
        ));
    }
    Ok(())
}
fn eligible(s: &str) -> bool {
    let (ranges, _, _, _) = crate::prose_ranges(s);
    let mut covered = vec![false; s.len()];
    for r in ranges {
        for x in &mut covered[r] {
            *x = true;
        }
    }
    s.char_indices()
        .all(|(i, c)| c.is_whitespace() || covered[i])
        && !s.contains(['「', '」', '『', '』', '"', '\''])
}
/// Import an existing profile explicitly and append explicit corrections.
pub fn learn(input: &Value) -> Result<Value, Error> {
    let mut p = profile(input.get("profile"))?;
    let examples = input
        .get("examples")
        .and_then(Value::as_array)
        .ok_or(Error("examplesには配列を指定してください。"))?;
    if examples.len() > MAX_RULES {
        return Err(Error("修正例は64件までです。"));
    }
    let mut ignored = 0;
    for e in examples {
        if !e
            .as_object()
            .is_some_and(|o| o.keys().all(|k| k == "before" || k == "after"))
        {
            return Err(Error("修正例に未対応の項目があります。"));
        }
        let a = e
            .get("before")
            .and_then(Value::as_str)
            .ok_or(Error("beforeには文章を指定してください。"))?;
        let b = e
            .get("after")
            .and_then(Value::as_str)
            .ok_or(Error("afterには文章を指定してください。"))?;
        validate(a, b)?;
        // Only layout and punctuation grouping with unchanged letters are learned.
        // This deliberately excludes register/politeness and lexical paraphrases.
        let letters = |s: &str| {
            s.chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
        };
        if !eligible(a) || !eligible(b) || letters(a) != letters(b) {
            ignored += 1;
            continue;
        }
        if !p.rules.iter().any(|r| r.before == a && r.after == b) {
            if p.rules.len() == MAX_RULES {
                return Err(Error("設定は64件までです。"));
            }
            p.rules.push(Correction {
                before: a.into(),
                after: b.into(),
                kind: kind(a, b).into(),
            });
        }
    }
    profile(Some(&json!(p)))?;
    Ok(
        json!({"profile":p,"ignored_examples":ignored,"interpretation":"指定された修正例から、改行位置や空行の間隔、句読点の変更を学びます。\n意味や敬語は推定しないため、引き継ぐ場合は返されたprofileを保存してください。"}),
    )
}
/// Inspect exact learned forms without modifying source or protected Markdown.
pub fn review(input: &Value) -> Result<Value, Error> {
    let text = input
        .get("text")
        .and_then(Value::as_str)
        .ok_or(Error("textには文章を指定してください。"))?;
    check(text)?;
    let p = profile(Some(
        input.get("profile").ok_or(Error("profileが必要です。"))?,
    ))?;
    let ranges = protected_prose(text);
    let (mut findings, mut limited) = layout_findings(text, &p);

    let mut seen = std::collections::BTreeSet::new();
    for r in &p.rules {
        if r.kind == "layout" || !seen.insert(r.before.clone()) {
            continue;
        }
        if !eligible(&r.before) || !eligible(&r.after) {
            continue;
        }
        for (start, _) in text.match_indices(&r.before) {
            let end = start + r.before.len();
            let allowed = text[start..end]
                .char_indices()
                .all(|(i, c)| c.is_whitespace() || ranges.iter().any(|r| r.contains(&(start + i))));
            if !allowed {
                continue;
            }
            if findings.len() >= 128 {
                limited = true;
                break;
            }
            let choices: std::collections::BTreeSet<_> = p
                .rules
                .iter()
                .filter(|x| x.before == r.before && x.kind != "layout")
                .map(|x| x.after.clone())
                .collect();
            findings.push(json!({"choices":choices,"kind":if choices.len()>1 {"explicit_wording_conflict"} else {r.kind.as_str()},"start":crate::locate(text,start),"end":crate::locate(text,end),"suggestion":if choices.len()==1 {Some(r.after.as_str())} else {None},"basis":"明示修正の原文との完全一致です。"}));
        }
    }
    Ok(
        json!({"findings":findings,"findings_limited":limited,"interpretation":"改行位置と空行による距離を分けて照合し、句読点の変更は修正前の文章と一致する場合に候補を示します。\n意味を解析するものではなく、原文も変更しません。"}),
    )
}
fn layout_findings(text: &str, p: &StyleProfile) -> (Vec<Value>, bool) {
    use std::collections::{BTreeMap, BTreeSet};
    fn gaps(s: &str) -> Vec<(usize, usize, String, String, String)> {
        let c: Vec<_> = s
            .char_indices()
            .filter(|(_, c)| !c.is_whitespace())
            .collect();
        (1..c.len())
            .map(|i| {
                let a = c[i - 1].0 + c[i - 1].1.len_utf8();
                let b = c[i].0;
                (
                    a,
                    b,
                    c[i.saturating_sub(4)..i].iter().map(|(_, c)| c).collect(),
                    c[i..(i + 4).min(c.len())].iter().map(|(_, c)| c).collect(),
                    s[a..b].replace("\r\n", "\n"),
                )
            })
            .collect()
    }
    let mut learned: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    for r in &p.rules {
        if compact(&r.before) != compact(&r.after) {
            continue;
        }
        for (a, b) in gaps(&r.before).iter().zip(gaps(&r.after)) {
            if a.4 != b.4 {
                learned.entry((b.2, b.3)).or_default().insert(b.4);
            }
        }
    }
    let ranges = protected_prose(text);
    let mut out = vec![];
    for (a, b, l, r, g) in gaps(text) {
        let options: BTreeSet<String> = learned
            .iter()
            .filter(|((left, right), _)| {
                l.ends_with(left.as_str()) && r.starts_with(right.as_str())
            })
            .flat_map(|(_, v)| v.iter().cloned())
            .collect();
        if options.is_empty() {
            continue;
        }
        let prev = text[..a].char_indices().last().map(|(i, _)| i).unwrap_or(0);
        if !ranges.iter().any(|r| r.contains(&prev)) || !ranges.iter().any(|r| r.contains(&b)) {
            continue;
        }
        if options.len() == 1 && options.contains(&g) {
            continue;
        }
        let choices:Vec<_>=options.iter().map(|s| {let n=s.matches('\n').count(); json!({"whitespace":s,"spacing":if n==0 {"same_line"} else if n==1 {"line_break"} else {"paragraph_distance"},"blank_lines":n.saturating_sub(1)})}).collect();
        if out.len() == 128 {
            return (out, true);
        }
        out.push(json!({"kind":if options.len()>1 {"layout_conflict"} else {"layout_boundary"},"start":crate::locate(text,a),"end":crate::locate(text,b),"choices":choices}));
    }
    (out, false)
}
fn protected_prose(text: &str) -> Vec<std::ops::Range<usize>> {
    let (ranges, _, _, _) = crate::prose_ranges(text);
    let mut protected = crate::quote_ranges_with_ascii(text, &ranges, true);
    protected.extend(crate::url_re().find_iter(text).map(|m| m.range()));
    crate::editable_ranges(ranges, &mut protected)
}
