//! Practical Japanese writing feedback. Rules are review prompts, not authorship tests.
pub mod rpc;
pub mod style;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, ops::Range, sync::OnceLock};

pub const MAX_TEXT_BYTES: usize = 128 * 1024;
const MAX_FINDINGS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub &'static str);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for Error {}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Location {
    pub byte: usize,
    pub character: usize,
    pub line: usize,
    pub column: usize,
}
fn locate(text: &str, byte: usize) -> Location {
    let head = &text[..byte];
    Location {
        byte,
        character: head.chars().count(),
        line: head.bytes().filter(|c| *c == b'\n').count() + 1,
        column: head.rsplit('\n').next().unwrap_or("").chars().count() + 1,
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Finding {
    pub rule: String,
    pub start: Location,
    pub end: Location,
    pub explanation: String,
    pub review_action: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Metrics {
    pub visible_characters: usize,
    pub sentence_units: usize,
    pub sentence_lengths: Vec<usize>,
    pub mean_sentence_length: f64,
    pub comma_count: usize,
    pub kanji_count: usize,
    pub kanji_ratio: f64,
    pub headings: usize,
    pub lists: usize,
    pub strong_spans: usize,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Analysis {
    pub metrics: Metrics,
    pub findings: Vec<Finding>,
    pub findings_limited: bool,
    pub interpretation: String,
}

fn check(text: &str) -> Result<(), Error> {
    if text.len() > MAX_TEXT_BYTES {
        Err(Error("文章はUTF-8で128KiBまでです。"))
    } else {
        Ok(())
    }
}
fn numeric_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"[+-]?[0-9０-９一二三四五六七八九十百千万億兆〇零]+(?:[.,．][0-9０-９]+)*(?:時間|秒|分|日|年|月|円|回|件|人|個|匹|本|枚|台|%|％)*(?:以上|以下|未満|超)?").expect("constant numeric pattern")
    })
}
fn quote_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new("「[^」\\r\\n]*」|『[^』\\r\\n]*』").expect("constant quote pattern")
    })
}
fn parser(text: &str) -> Parser<'_> {
    Parser::new_ext(
        text.strip_prefix('\u{feff}').unwrap_or(text),
        Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
            | Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS,
    )
}
fn identifiers() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"Q[0-9]+|branch-[A-Za-z0-9_-]+").expect("constant identifier pattern")
    })
}
fn url_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r#"https?://[A-Za-z0-9._~:/?#\[\]@!$&'()*+,;=%-]+"#)
            .expect("constant ASCII URL pattern")
    })
}
fn kanji_run_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"[一-龠々]{14,}").expect("constant kanji sequence"))
}

/// CommonMark provides source ranges. Code, links, quotations and headings are not edited.
fn prose_ranges(text: &str) -> (Vec<Range<usize>>, usize, usize, usize) {
    let mut ranges = Vec::new();
    let prefix_bytes = if text.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        0
    };
    let (mut excluded, mut headings, mut lists, mut strong) = (0usize, 0, 0, 0);
    for (event, range) in parser(text).into_offset_iter() {
        match event {
            Event::Start(tag) => match tag {
                Tag::CodeBlock(_)
                | Tag::BlockQuote(_)
                | Tag::Link { .. }
                | Tag::Image { .. }
                | Tag::MetadataBlock(_) => excluded += 1,
                Tag::Heading { .. } => {
                    headings += 1;
                    excluded += 1;
                }
                Tag::List(_) => lists += 1,
                Tag::Strong => strong += 1,
                _ => {}
            },
            Event::End(
                TagEnd::CodeBlock
                | TagEnd::BlockQuote(_)
                | TagEnd::Link
                | TagEnd::Image
                | TagEnd::MetadataBlock(_)
                | TagEnd::Heading(_),
            ) => excluded = excluded.saturating_sub(1),
            Event::Text(_) if excluded == 0 => {
                ranges.push(range.start + prefix_bytes..range.end + prefix_bytes)
            }
            _ => {}
        }
    }
    (ranges, headings, lists, strong)
}
fn visible(text: &str) -> String {
    visible_with_breaks(text, true)
}
fn visible_with_breaks(text: &str, count_line_breaks: bool) -> String {
    let mut output = String::new();
    let mut excluded = 0usize;
    for event in parser(text) {
        match event {
            Event::Start(Tag::CodeBlock(_) | Tag::MetadataBlock(_)) => excluded += 1,
            Event::End(TagEnd::CodeBlock | TagEnd::MetadataBlock(_)) => {
                excluded = excluded.saturating_sub(1)
            }
            Event::Text(t) if excluded == 0 => output.push_str(&t),
            Event::SoftBreak | Event::HardBreak if !count_line_breaks => output.push(' '),
            Event::SoftBreak
            | Event::HardBreak
            | Event::End(TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::Item) => {
                output.push('\n')
            }
            _ => {}
        }
    }
    output
}
fn overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}
/// Remove protected islands before rule matching rather than discarding a whole finding.
fn editable_ranges(ranges: Vec<Range<usize>>, protected: &mut [Range<usize>]) -> Vec<Range<usize>> {
    protected.sort_by_key(|range| range.start);
    let mut editable = Vec::new();
    for range in ranges {
        let mut cursor = range.start;
        for island in protected.iter().filter(|island| overlaps(&range, island)) {
            if island.start > cursor {
                editable.push(cursor..island.start.min(range.end));
            }
            cursor = cursor.max(island.end.min(range.end));
            if cursor == range.end {
                break;
            }
        }
        if cursor < range.end {
            editable.push(cursor..range.end);
        }
    }
    editable
}

pub fn analyze(text: &str) -> Result<Analysis, Error> {
    check(text)?;
    let (prose, headings, lists, strong_spans) = prose_ranges(text);
    let mut quotes: Vec<_> = quote_re()
        .find_iter(text)
        .chain(url_re().find_iter(text))
        .map(|m| m.range())
        .collect();
    let prose = editable_ranges(prose, &mut quotes);
    let mut findings = Vec::new();
    let mut limited = false;
    let mut add = |range: Range<usize>, rule: &str, why: &str, action: &str| {
        if findings.len() >= MAX_FINDINGS {
            limited = true;
            return;
        }
        findings.push(Finding {
            rule: rule.into(),
            start: locate(text, range.start),
            end: locate(text, range.end),
            explanation: why.into(),
            review_action: action.into(),
        });
    };
    let plain = visible(text);
    if plain
        .lines()
        .next()
        .is_some_and(|line| line.starts_with("状態: 完了") || line.starts_with("状態：完了"))
    {
        for r in &prose {
            let s = &text[r.clone()];
            for marker in ["未完了", "まだ実行していません", "これから確認"] {
                for (at, _) in s.match_indices(marker) {
                    add(
                        r.start + at..r.start + at + marker.len(),
                        "reader.status_alignment",
                        "完了のラベルと本文の残作業を照合してください。",
                        "完了条件を満たした範囲と、残っている作業を分けて状態を示してください。",
                    );
                }
            }
        }
    }
    for range in &prose {
        let s = &text[range.clone()];
        let character_length = s.chars().count();
        for m in kanji_run_re().find_iter(s) {
            add(
                range.start + m.start()..range.start + m.end(),
                "reader.noun_relationship",
                "漢字が長く続いています。名前や専門語か、動作が名詞に埋もれた箇所かを確認します。",
                "名前は保ち、動作の説明なら確定した主語と動詞で書けるか確認してください。",
            );
        }
        for m in identifiers().find_iter(s) {
            if character_length < 32 {
                add(
                    range.start + m.start()..range.start + m.end(),
                    "reader.identifier_meaning",
                    "短い説明で内部の番号を使っています。読み手が内容を分かるか確認します。",
                    "参照番号を残しても、何を選ぶか、何が起きるかを具体的に添えてください。",
                );
            }
        }
        for word in ["ローカル", "既定", "フォールバック", "見送り"] {
            for (at, _) in s.match_indices(word) {
                add(
                    range.start + at..range.start + at + word.len(),
                    "reader.ambiguous_term",
                    "読み手によって指す場所や動作が変わる語です。",
                    "何を指すかが前後から分かるなら残し、不明なら具体的な説明を添えてください。",
                );
            }
        }
        if s.matches('の').count() >= 4 && character_length >= 24 {
            add(
                range.clone(),
                "reader.noun_relationship",
                "名詞の関係を読み戻す負担がないか確認する候補です。",
                "確定している主体と動作を動詞で書けるか確認し、名前や引用は変えないでください。",
            );
        }
        for marker in ["ご報告いたします", "推進してまいります", "展開を実施"]
        {
            for (at, _) in s.match_indices(marker) {
                add(
                    range.start + at..range.start + at + marker.len(),
                    "reader.action_visibility",
                    "書き手の態度より、実際の結果や操作を先に伝えられるか確認します。",
                    "同じ事実を短い主語と動詞で伝えられるか検討してください。丁寧さが必要な文脈は保ってください。",
                );
            }
        }
        for unit in s.split_inclusive(['。', '！', '？', '\n']) {
            if unit.chars().count() > 80 {
                let at = unit.as_ptr() as usize - s.as_ptr() as usize;
                add(
                    range.start + at..range.start + at + unit.len(),
                    "reader.long_relationship",
                    "長い文の中で条件と動作の対応を追えるか確認します。",
                    "理由・条件・時系列を保てる場合にだけ分けてください。長さだけで情報を削らないでください。",
                );
            }
        }
    }
    let lead = prose.first().map(|r| &text[r.clone()]).unwrap_or("");
    if (lead.starts_with("背景として") || lead.starts_with("今回の背景"))
        && let Some(r) = prose.first()
    {
        add(
            r.clone(),
            "reader.result_location",
            "結果や依頼より先に背景が続いていないか確認します。",
            "必要な操作と結果を最初に置けるか確認してください。前提を先に伝える必要がある文は残してください。",
        );
    }
    findings.sort_by_key(|f| (f.start.byte, f.end.byte));
    let chars = plain.chars().filter(|c| !c.is_whitespace()).count();
    let sentence_text = visible_with_breaks(text, false);
    let sentence_lengths: Vec<usize> = sentence_text
        .split(['。', '！', '？', '\n'])
        .map(|s| s.chars().filter(|c| !c.is_whitespace()).count())
        .filter(|n| *n > 0)
        .collect();
    let kanji_count = plain
        .chars()
        .filter(|c| matches!(*c as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0x20000..=0x323AF))
        .count();
    let metrics = Metrics {
        visible_characters: chars,
        sentence_units: sentence_lengths.len(),
        mean_sentence_length: if sentence_lengths.is_empty() {
            0.0
        } else {
            sentence_lengths.iter().sum::<usize>() as f64 / sentence_lengths.len() as f64
        },
        sentence_lengths,
        comma_count: plain.matches('、').count(),
        kanji_count,
        kanji_ratio: if chars == 0 {
            0.0
        } else {
            kanji_count as f64 / chars as f64
        },
        headings,
        lists,
        strong_spans,
    };
    Ok(Analysis {
        metrics,
        findings,
        findings_limited: limited,
        interpretation:
            "指摘は読み手の負担を確認する候補です。作者が人間かAIか、意味が正しいかを判定しません。"
                .into(),
    })
}

pub fn features() -> Value {
    json!({"version":env!("CARGO_PKG_VERSION"),"max_text_bytes":MAX_TEXT_BYTES,"automatic_rewrite":false,"authorship_detection":false,"rules":["reader.result_location","reader.identifier_meaning","reader.ambiguous_term","reader.noun_relationship","reader.action_visibility","reader.long_relationship","reader.status_alignment"],"metrics":["visible_characters","sentence_units","sentence_lengths","mean_sentence_length","comma_count","kanji_count","kanji_ratio","headings","lists","strong_spans"]})
}
pub fn plan(text: &str, context: &str) -> Result<Value, Error> {
    check(context)?;
    let report = analyze(text)?;
    Ok(
        json!({"original":text,"context":context,"analysis":report,"performs_rewrite":false,"steps":[
            "読み手、文章の目的、伝える結果、求める操作を提供された情報から確認する。",
            "元の事実、数量、期限、条件、例外、否定、比較の向き、確かさ、依頼や決定の強さを控える。",
            "指摘が読みにくさを生むか前後を読んで確かめ、問題がある箇所だけを直す。自然な表現は残す。",
            "何が起きるかを最初に伝えられるか検討する。主語と動作を分かる範囲で書き、内部の番号だけで説明しない。",
            "分からない主体や理由を作らない。架空の実績や経験を足さず、未確定の点は未確定のまま示す。",
            "敬語・ため口は評価せず、意味のつながる語句・節・文のまとまりを保つ。改行位置と空行による距離を別々に検討し、句点ごとの機械的な分割を避ける。",
            "コード、URL、引用、名前、正確な値を保つ。改稿後に原文と見比べ、変わった意味を確認する。"
        ]}),
    )
}
fn numeric_fragments(text: &str) -> Vec<String> {
    numeric_re()
        .find_iter(text)
        .map(|m| m.as_str().to_owned())
        .collect()
}
fn meaning_markers(text: &str) -> BTreeMap<&'static str, usize> {
    [
        "場合",
        "だけ",
        "除く",
        "しません",
        "しない",
        "かもしれ",
        "必要",
        "必ず",
        "ください",
        "予定",
        "できる",
        "できます",
        "まで",
        "より",
    ]
    .into_iter()
    .map(|s| (s, text.matches(s).count()))
    .collect()
}
pub fn compare(original: &str, rewrite: &str) -> Result<Value, Error> {
    check(original)?;
    check(rewrite)?;
    let mut prefix = 0;
    for (a, b) in original.chars().zip(rewrite.chars()) {
        if a != b {
            break;
        }
        prefix += a.len_utf8();
    }
    let (mut o_end, mut r_end) = (original.len(), rewrite.len());
    for (a, b) in original[prefix..]
        .chars()
        .rev()
        .zip(rewrite[prefix..].chars().rev())
    {
        if a != b {
            break;
        }
        o_end -= a.len_utf8();
        r_end -= b.len_utf8();
    }
    Ok(
        json!({"changed":original != rewrite,"changed_region":{"original_start":locate(original,prefix),"original_end":locate(original,o_end),"rewrite_start":locate(rewrite,prefix),"rewrite_end":locate(rewrite,r_end)},
        "numeric_fragments_changed":numeric_fragments(original)!=numeric_fragments(rewrite),"numeric_fragments_original":numeric_fragments(original),"numeric_fragments_rewrite":numeric_fragments(rewrite),
        "listed_marker_counts_changed":meaning_markers(original)!=meaning_markers(rewrite),"original_markers":meaning_markers(original),"rewrite_markers":meaning_markers(rewrite),
        "comparison_scope":"数字・漢数字と列挙した直後の単位・比較語を出現順で照合します。数量表現を網羅しません。語の件数は列挙した語だけで、意味の保持を確定しません。",
        "original_analysis":analyze(original)?,"rewrite_analysis":analyze(rewrite)?,"semantic_equality":"not_established","review":"差分は一つの外側範囲です。語の数が同じでも条件や主体が変わることがあります。原文と全文を照合してください。"}),
    )
}
pub fn dispatch(input: &Value) -> Result<Value, Error> {
    let obj = input.as_object().ok_or(Error("入力はJSON objectです。"))?;
    let action = obj
        .get("action")
        .and_then(Value::as_str)
        .ok_or(Error("actionを指定してください。"))?;
    let fields: &[&str] = match action {
        "analyze" => &["action", "text"],
        "compare" => &["action", "original", "rewrite"],
        "plan" => &["action", "text", "context"],
        "features" => &["action", "text"],
        "style_learn" => &["action", "examples", "profile"],
        "style_review" => &["action", "text", "profile"],
        _ => return Err(Error("未対応のactionです。")),
    };
    if obj.keys().any(|key| !fields.contains(&key.as_str())) {
        return Err(Error("未対応の項目があります。"));
    }
    let required = |key| {
        obj.get(key)
            .and_then(Value::as_str)
            .ok_or(Error("必要な文章を文字列で指定してください。"))
    };
    match action {
        "analyze" => serde_json::to_value(analyze(required("text")?)?)
            .map_err(|_| Error("結果を変換できません。")),
        "compare" => compare(required("original")?, required("rewrite")?),
        "style_learn" => style::learn(input),
        "style_review" => style::review(input),
        "plan" => {
            let context = if obj.contains_key("context") {
                required("context")?
            } else {
                ""
            };
            plan(required("text")?, context)
        }
        _ => {
            if obj.contains_key("text") {
                Ok(json!({"definitions":features(),"metrics":analyze(required("text")?)?.metrics}))
            } else {
                Ok(features())
            }
        }
    }
}
