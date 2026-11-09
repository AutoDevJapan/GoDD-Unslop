//! Portable skill installation, with explicit destination and no host configuration edits.
use crate::Error;
use std::{fs, path::Path};
pub const CONTENT: &str = include_str!("../docs/distribution/godd-unslop-SKILL.md");
fn section(name: &str) -> String {
    let content = CONTENT.replace("\r\n", "\n");
    let start_marker = format!("<!-- godd-unslop:{name}:start -->");
    let end_marker = format!("<!-- godd-unslop:{name}:end -->");
    let start = content.find(&start_marker).expect("guidance start") + start_marker.len();
    let end = content.find(&end_marker).expect("guidance end");
    content[start..end].trim().to_string()
}
/// Line-break and blank-line guidance shared by the Skill and the rewrite prompt.
pub fn layout_guidance() -> String {
    section("layout")
}
/// Topic, viewpoint, tense, certainty and feeling guidance shared by the Skill and the rewrite prompt.
pub fn nuance_guidance() -> String {
    section("nuance")
}
/// Register guidance shared by the Skill and the rewrite prompt.
pub fn register_guidance() -> String {
    section("register")
}
/// Recipient, closeness, tone and emoji guidance shared by the Skill and the rewrite prompt.
pub fn recipient_guidance() -> String {
    section("recipient")
}
pub fn install(root: &Path) -> Result<std::path::PathBuf, Error> {
    let directory = root.join("godd-unslop");
    fs::create_dir_all(&directory).map_err(|_| Error("Skillの保存先を作れません。"))?;
    let target = directory.join("SKILL.md");
    if target.exists() {
        let current =
            fs::read_to_string(&target).map_err(|_| Error("既存Skillを確認できません。"))?;
        if current == CONTENT {
            return Ok(target);
        }
        return Err(Error(
            "異なる既存Skillがあります。保存先を分けるか内容を確認してください。",
        ));
    }
    use std::io::Write;
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|_| Error("Skillを作れません。"))?;
    f.write_all(CONTENT.as_bytes())
        .map_err(|_| Error("Skillを書き込めません。"))?;
    f.sync_all().map_err(|_| Error("Skillを保存できません。"))?;
    if fs::read_to_string(&target).map_err(|_| Error("Skillを読み返せません。"))? != CONTENT
    {
        return Err(Error("保存したSkillが一致しません。"));
    }
    Ok(target)
}
