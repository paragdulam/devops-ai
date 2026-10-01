use crate::project_kind::ProjectKind;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

/// Where a detected tool requirement came from — shown as a badge in the UI
/// so the user can tell a README guess from an exact version-file pin.
#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ToolSource {
    Readme,
    ToolVersions,
    MiseToml,
    Nvmrc,
    NodeVersion,
    PythonVersion,
    RubyVersion,
    JavaVersion,
    GoMod,
    RustToolchain,
    PackageJson,
    Pubspec,
    KindDefault,
    User,
}

/// One tool for mise to install on the VM: `mise use --global {tool}@{version}`.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequirement {
    pub tool: String,
    pub version: String,
    pub source: ToolSource,
}

/// Repo-relative files read for tool versions, on top of
/// `project_kind::MARKER_FILES` (which already covers `package.json` and
/// `pubspec.yaml`). README variants come first: the README is the primary
/// source, version files only fill in tools it doesn't mention.
pub const TOOLCHAIN_FILES: &[&str] = &[
    "README.md",
    "readme.md",
    "Readme.md",
    ".tool-versions",
    "mise.toml",
    ".mise.toml",
    ".nvmrc",
    ".node-version",
    ".python-version",
    ".ruby-version",
    ".java-version",
    "go.mod",
    "rust-toolchain.toml",
    "rust-toolchain",
];

/// mise tool name -> the words a README or `.tool-versions` uses for it.
/// Doubles as the allowlist `validate` enforces: nothing outside this table
/// is ever templated into the playbook.
const TOOLS: &[(&str, &[&str])] = &[
    ("node", &["node", "node.js", "nodejs"]),
    ("python", &["python", "python3"]),
    ("java", &["java", "jdk", "openjdk"]),
    ("go", &["go", "golang"]),
    ("rust", &["rust", "rustc"]),
    ("ruby", &["ruby"]),
    ("flutter", &["flutter"]),
    ("gradle", &["gradle"]),
    ("maven", &["maven"]),
    ("pnpm", &["pnpm"]),
    ("yarn", &["yarn"]),
    ("bun", &["bun"]),
    ("deno", &["deno"]),
    ("terraform", &["terraform"]),
    ("kubectl", &["kubectl"]),
    ("helm", &["helm"]),
    ("aws-cli", &["awscli", "aws-cli"]),
];

/// Tools whose name is also a common English word, so a bare
/// "go 2 levels up" in prose doesn't become a Go requirement: free-text
/// matches for these need a dotted version (`go 1.22`).
const AMBIGUOUS: &[&str] = &["go"];

pub fn tool_names() -> impl Iterator<Item = &'static str> {
    TOOLS.iter().map(|(name, _)| *name)
}

fn canonical(word: &str) -> Option<&'static str> {
    let word = word.to_ascii_lowercase();
    TOOLS
        .iter()
        .find(|(name, aliases)| *name == word || aliases.contains(&word.as_str()))
        .map(|(name, _)| *name)
}

static VERSION_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+(?:\.\d+){0,2}").unwrap());
static VALID_VERSION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9._+-]{1,40}$").unwrap());

/// Reduces whatever a README or version file says to something mise
/// accepts: `>=18.0.0` -> `18.0.0`, `v20` -> `20`, `lts/*` -> `lts`,
/// `temurin-17.0.9` -> `17.0.9`. `None` for anything unrecognisable.
fn normalize_version(raw: &str) -> Option<String> {
    let v = raw
        .trim()
        .trim_matches(|c| c == '"' || c == '\'' || c == '`')
        .trim_start_matches(|c: char| "<>=^~ v".contains(c));
    let lower = v.to_ascii_lowercase();
    if lower.starts_with("lts") {
        return Some("lts".to_string());
    }
    if matches!(lower.as_str(), "latest" | "stable" | "beta" | "nightly") {
        return Some(lower);
    }
    if let Some(m) = VERSION_RE.find(v) {
        return Some(m.as_str().to_string());
    }
    // Vendor-prefixed identifiers (`temurin-17.0.9`, `17.0.9-tem`, `go1.22.3`).
    let digits = v.find(|c: char| c.is_ascii_digit())?;
    VERSION_RE
        .find(&v[digits..])
        .map(|m| m.as_str().to_string())
}

/// mise's bare `java@17` resolves to the `openjdk` vendor, which stops
/// publishing a release line once it leaves GA — Temurin keeps LTS lines
/// available, so numeric Java versions are pinned to it.
fn mise_version(tool: &str, version: String) -> String {
    if tool == "java" && version.starts_with(|c: char| c.is_ascii_digit()) {
        format!("temurin-{version}")
    } else {
        version
    }
}

fn requirement(tool: &str, raw_version: &str, source: ToolSource) -> Option<ToolRequirement> {
    let version = normalize_version(raw_version)?;
    Some(ToolRequirement {
        tool: tool.to_string(),
        version: mise_version(tool, version),
        source,
    })
}

/// Adds `req` unless that tool is already present — callers push in
/// priority order, so the first source to mention a tool wins.
fn push(out: &mut Vec<ToolRequirement>, req: Option<ToolRequirement>) {
    if let Some(req) = req {
        if !out.iter().any(|r| r.tool == req.tool) {
            out.push(req);
        }
    }
}

fn alias_pattern() -> String {
    let mut aliases: Vec<&str> = TOOLS.iter().flat_map(|(_, a)| a.iter().copied()).collect();
    // Longest first so `node.js` wins over `node`, `python3` over `python`.
    aliases.sort_by_key(|a| std::cmp::Reverse(a.len()));
    aliases
        .iter()
        .map(|a| regex::escape(a))
        .collect::<Vec<_>>()
        .join("|")
}

/// Version-manager commands in setup instructions — the most precise thing
/// a README says, so they're matched before free-text mentions.
static INSTALL_COMMANDS: LazyLock<Vec<(Regex, Option<&'static str>)>> = LazyLock::new(|| {
    let v = r"([^\s`'\x22]+)";
    [
        (
            format!(r"(?i)\bnvm\s+(?:install|use|alias\s+default)\s+(--lts|{v})"),
            Some("node"),
        ),
        (
            format!(r"(?i)\bvolta\s+(?:install|pin)\s+node@{v}"),
            Some("node"),
        ),
        (
            format!(r"(?i)\bfnm\s+(?:install|use)\s+(--lts|{v})"),
            Some("node"),
        ),
        (
            format!(r"(?i)\bpyenv\s+(?:install|local|global)\s+{v}"),
            Some("python"),
        ),
        (
            format!(r"(?i)\buv\s+python\s+(?:install|pin)\s+{v}"),
            Some("python"),
        ),
        (
            format!(r"(?i)\brbenv\s+(?:install|local|global)\s+{v}"),
            Some("ruby"),
        ),
        (format!(r"(?i)\brvm\s+(?:install|use)\s+{v}"), Some("ruby")),
        (format!(r"(?i)\bgoenv\s+install\s+{v}"), Some("go")),
        (
            format!(r"(?i)\brustup\s+(?:default|install|toolchain\s+install)\s+{v}"),
            Some("rust"),
        ),
        (
            format!(r"(?i)\bfvm\s+(?:install|use|global)\s+{v}"),
            Some("flutter"),
        ),
        (
            format!(r"(?i)\bsdk\s+(?:install|use|default)\s+(java|gradle|maven)\s+{v}"),
            None,
        ),
        (
            format!(r"(?i)\b(?:mise|rtx)\s+(?:use|install)(?:\s+(?:-g|--global))?\s+([\w.-]+)@{v}"),
            None,
        ),
        (
            format!(r"(?i)\basdf\s+(?:install|global|local|set)\s+([\w.-]+)\s+{v}"),
            None,
        ),
    ]
    .into_iter()
    .map(|(pattern, tool)| (Regex::new(&pattern).unwrap(), tool))
    .collect()
});

/// `Node.js 20`, `Python >= 3.11`, `JDK 17`, `node v18.19.0`, `Go 1.22+`,
/// `Flutter (3.22.x)`, `Node.js version 20`.
static FREE_TEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)(?:^|[^\w.-])({aliases})[\s:*`()\[\]]*(?:version\s*|v\.?\s*)?(?:>=|=>|>|=|\^|~|@)?\s*v?(\d+(?:\.\d+){{0,2}})\b",
        aliases = alias_pattern()
    ))
    .unwrap()
});

/// A list item in a Prerequisites-style section that *starts* with a tool
/// name (`- Node.js`, `* [Flutter](https://…)`, `1. **Java**`), for tools
/// listed with no version at all — installed at `latest`.
static LIST_ITEM_TOOL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)^\s*(?:[-*+]|\d+\.)\s+[\[*_`]*({aliases})(?:$|[^\w.-])",
        aliases = alias_pattern()
    ))
    .unwrap()
});

static PREREQ_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*#{1,6}\s+.*\b(prerequisites?|requirements?|dependencies|setup|installation|getting started|tools|tech stack|built with)\b").unwrap()
});

pub fn parse_readme(text: &str) -> Vec<ToolRequirement> {
    let mut out = Vec::new();

    for (re, fixed_tool) in INSTALL_COMMANDS.iter() {
        for caps in re.captures_iter(text) {
            let (tool, version) = match fixed_tool {
                Some(tool) => (Some(*tool), caps.get(1).map_or("", |m| m.as_str())),
                None => (
                    caps.get(1).and_then(|m| canonical(m.as_str())),
                    caps.get(2).map_or("", |m| m.as_str()),
                ),
            };
            let version = if version == "--lts" { "lts" } else { version };
            if let Some(tool) = tool {
                push(&mut out, requirement(tool, version, ToolSource::Readme));
            }
        }
    }

    for caps in FREE_TEXT.captures_iter(text) {
        let Some(tool) = canonical(&caps[1]) else {
            continue;
        };
        let version = &caps[2];
        if AMBIGUOUS.contains(&tool) && !version.contains('.') {
            continue;
        }
        push(&mut out, requirement(tool, version, ToolSource::Readme));
    }

    let mut in_prereqs = false;
    for line in text.lines() {
        if line.trim_start().starts_with('#') {
            in_prereqs = PREREQ_HEADING.is_match(line);
            continue;
        }
        if !in_prereqs {
            continue;
        }
        if let Some(caps) = LIST_ITEM_TOOL.captures(line) {
            if let Some(tool) = canonical(&caps[1]) {
                push(&mut out, requirement(tool, "latest", ToolSource::Readme));
            }
        }
    }

    out
}

fn first_line(content: &str) -> &str {
    content
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .unwrap_or("")
}

static TOML_STRING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"["']([^"']+)["']"#).unwrap());
static GO_MOD_TOOLCHAIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^toolchain\s+go(\S+)").unwrap());
static GO_MOD_GO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^go\s+(\S+)").unwrap());
static RUST_CHANNEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?m)^\s*channel\s*=\s*["']([^"']+)["']"#).unwrap());
static PUBSPEC_FLUTTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^environment:\s*\n(?:[ \t]+.*\n)*?[ \t]+flutter:\s*["']?([^"'\n]+)"#).unwrap()
});

fn parse_tool_versions(content: &str, out: &mut Vec<ToolRequirement>) {
    for line in content.lines() {
        let mut parts = line.split_whitespace();
        let (Some(name), Some(version)) = (parts.next(), parts.next()) else {
            continue;
        };
        if name.starts_with('#') {
            continue;
        }
        if let Some(tool) = canonical(name) {
            push(out, requirement(tool, version, ToolSource::ToolVersions));
        }
    }
}

/// Just enough of mise.toml for the `[tools]` table: `node = "20"`,
/// `python = ["3.11"]`, `"go" = { version = "1.22" }`.
fn parse_mise_toml(content: &str, out: &mut Vec<ToolRequirement>) {
    let mut in_tools = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_tools = line == "[tools]";
            continue;
        }
        if !in_tools {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_matches(|c| c == '"' || c == '\'');
        let Some(tool) = canonical(key) else { continue };
        if let Some(caps) = TOML_STRING.captures(value) {
            push(out, requirement(tool, &caps[1], ToolSource::MiseToml));
        }
    }
}

fn parse_package_json(content: &str, out: &mut Vec<ToolRequirement>) {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(content) else {
        return;
    };
    // `"packageManager": "pnpm@9.1.0+sha512…"` (Corepack) is an exact pin.
    if let Some((name, version)) = json["packageManager"]
        .as_str()
        .and_then(|s| s.split_once('@'))
    {
        if let Some(tool) = canonical(name) {
            let version = version.split('+').next().unwrap_or(version);
            push(out, requirement(tool, version, ToolSource::PackageJson));
        }
    }
    if let Some(engines) = json["engines"].as_object() {
        for (name, range) in engines {
            if let (Some(tool), Some(range)) = (canonical(name), range.as_str()) {
                push(out, requirement(tool, range, ToolSource::PackageJson));
            }
        }
    }
}

pub fn parse_version_files(files: &HashMap<&str, String>) -> Vec<ToolRequirement> {
    let mut out = Vec::new();
    let file = |name: &str| files.get(name).map(String::as_str);

    if let Some(c) = file(".tool-versions") {
        parse_tool_versions(c, &mut out);
    }
    for name in ["mise.toml", ".mise.toml"] {
        if let Some(c) = file(name) {
            parse_mise_toml(c, &mut out);
        }
    }

    let single_line: &[(&str, &str, ToolSource)] = &[
        (".nvmrc", "node", ToolSource::Nvmrc),
        (".node-version", "node", ToolSource::NodeVersion),
        (".python-version", "python", ToolSource::PythonVersion),
        (".ruby-version", "ruby", ToolSource::RubyVersion),
        (".java-version", "java", ToolSource::JavaVersion),
        ("rust-toolchain", "rust", ToolSource::RustToolchain),
    ];
    for (name, tool, source) in single_line {
        if let Some(c) = file(name) {
            push(&mut out, requirement(tool, first_line(c), *source));
        }
    }

    if let Some(c) = file("rust-toolchain.toml") {
        if let Some(caps) = RUST_CHANNEL.captures(c) {
            push(
                &mut out,
                requirement("rust", &caps[1], ToolSource::RustToolchain),
            );
        }
    }
    if let Some(c) = file("go.mod") {
        // `toolchain go1.22.3` is the exact toolchain; `go 1.22` the minimum.
        if let Some(caps) = GO_MOD_TOOLCHAIN
            .captures(c)
            .or_else(|| GO_MOD_GO.captures(c))
        {
            push(&mut out, requirement("go", &caps[1], ToolSource::GoMod));
        }
    }
    if let Some(c) = file("package.json") {
        parse_package_json(c, &mut out);
    }
    if let Some(c) = file("pubspec.yaml") {
        if let Some(caps) = PUBSPEC_FLUTTER.captures(c) {
            push(
                &mut out,
                requirement("flutter", &caps[1], ToolSource::Pubspec),
            );
        }
    }

    out
}

/// Builds the toolchain to preinstall. README first (the user's explicit
/// priority), then version files for anything the README didn't mention,
/// then defaults implied by the project kind.
pub fn detect(files: &HashMap<&str, String>, kind: ProjectKind) -> Vec<ToolRequirement> {
    let mut out = Vec::new();

    for name in ["README.md", "readme.md", "Readme.md"] {
        if let Some(readme) = files.get(name) {
            for req in parse_readme(readme) {
                push(&mut out, Some(req));
            }
            break;
        }
    }
    for req in parse_version_files(files) {
        push(&mut out, Some(req));
    }

    let default = |tool, version| requirement(tool, version, ToolSource::KindDefault);
    match kind {
        ProjectKind::Flutter => push(&mut out, default("flutter", "latest")),
        ProjectKind::ReactNative => {
            push(&mut out, default("node", "lts"));
            push(&mut out, default("java", "17"));
        }
        ProjectKind::Android => push(&mut out, default("java", "17")),
        ProjectKind::General => {
            if files.contains_key("package.json") {
                push(&mut out, default("node", "lts"));
            }
        }
    }

    out
}

pub fn detect_local(root: &Path, kind: ProjectKind) -> Vec<ToolRequirement> {
    let files = crate::project_kind::MARKER_FILES
        .iter()
        .chain(TOOLCHAIN_FILES)
        .filter_map(|&name| {
            std::fs::read_to_string(root.join(name))
                .ok()
                .map(|content| (name, content))
        })
        .collect();
    detect(&files, kind)
}

/// Gatekeeper before anything reaches the playbook: README text is
/// untrusted, and the UI lets the user type versions freely, so only
/// allowlisted tools with plain version strings get through.
pub fn validate(tools: &[ToolRequirement]) -> Result<(), String> {
    for t in tools {
        if !tool_names().any(|name| name == t.tool) {
            return Err(format!("Unsupported tool \"{}\"", t.tool));
        }
        if !VALID_VERSION_RE.is_match(&t.version) {
            return Err(format!("Invalid version \"{}\" for {}", t.version, t.tool));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(reqs: &[ToolRequirement]) -> Vec<(&str, &str)> {
        reqs.iter()
            .map(|r| (r.tool.as_str(), r.version.as_str()))
            .collect()
    }

    fn files(entries: &[(&'static str, &str)]) -> HashMap<&'static str, String> {
        entries.iter().map(|(n, c)| (*n, c.to_string())).collect()
    }

    #[test]
    fn readme_free_text_versions() {
        let readme = "\
You need Node.js 20 and Python >= 3.11.
Android builds require JDK 17. Use Go 1.22+ for the CLI.
";
        assert_eq!(
            pairs(&parse_readme(readme)),
            [
                ("node", "20"),
                ("python", "3.11"),
                ("java", "temurin-17"),
                ("go", "1.22")
            ]
        );
    }

    #[test]
    fn readme_install_commands_beat_prose() {
        let readme = "\
Works with Node 18 or later.

```bash
nvm install 20.11.1
pyenv install 3.12.1
sdk install java 21.0.2-tem
mise use -g terraform@1.9.5
asdf install nodejs 22.0.0
```
";
        assert_eq!(
            pairs(&parse_readme(readme)),
            [
                ("node", "20.11.1"),
                ("python", "3.12.1"),
                ("java", "temurin-21.0.2"),
                ("terraform", "1.9.5"),
            ]
        );
    }

    #[test]
    fn nvm_lts() {
        assert_eq!(pairs(&parse_readme("nvm install --lts")), [("node", "lts")]);
    }

    #[test]
    fn prerequisites_without_versions_install_latest() {
        let readme = "\
# My app

Let's go build something.

## Prerequisites

- [Flutter](https://flutter.dev)
- **Ruby**
- An AWS account

## Usage

- go to the dashboard
";
        assert_eq!(
            pairs(&parse_readme(readme)),
            [("flutter", "latest"), ("ruby", "latest")]
        );
    }

    #[test]
    fn bare_go_in_prose_is_ignored() {
        assert!(parse_readme("Go 2 directories up and run make").is_empty());
    }

    #[test]
    fn version_files() {
        let f = files(&[
            (
                ".tool-versions",
                "nodejs 20.11.0\ngolang 1.22.1\nunknowntool 1.0\n",
            ),
            (".python-version", "3.12.2\n"),
            ("rust-toolchain.toml", "[toolchain]\nchannel = \"1.79.0\"\n"),
            (
                "package.json",
                r#"{"packageManager": "pnpm@9.1.0+sha512.abc", "engines": {"node": ">=18"}}"#,
            ),
        ]);
        assert_eq!(
            pairs(&parse_version_files(&f)),
            [
                ("node", "20.11.0"),
                ("go", "1.22.1"),
                ("python", "3.12.2"),
                ("rust", "1.79.0"),
                ("pnpm", "9.1.0"),
            ]
        );
    }

    #[test]
    fn mise_toml_and_go_mod() {
        let f = files(&[
            (
                "mise.toml",
                "[env]\nFOO = \"1\"\n[tools]\nnode = \"22\"\n\"python\" = [\"3.11\"]\n",
            ),
            ("go.mod", "module x\n\ngo 1.21\n\ntoolchain go1.22.3\n"),
        ]);
        assert_eq!(
            pairs(&parse_version_files(&f)),
            [("node", "22"), ("python", "3.11"), ("go", "1.22.3")]
        );
    }

    #[test]
    fn pubspec_flutter_constraint() {
        let pubspec = "name: app\nenvironment:\n  sdk: \">=3.4.0 <4.0.0\"\n  flutter: \">=3.22.0\"\ndependencies:\n  flutter:\n    sdk: flutter\n";
        assert_eq!(
            pairs(&parse_version_files(&files(&[("pubspec.yaml", pubspec)]))),
            [("flutter", "3.22.0")]
        );
    }

    #[test]
    fn readme_wins_over_version_files() {
        let f = files(&[("README.md", "Requires Node.js 22."), (".nvmrc", "v18\n")]);
        let tools = detect(&f, ProjectKind::General);
        assert_eq!(pairs(&tools), [("node", "22")]);
        assert_eq!(tools[0].source, ToolSource::Readme);
    }

    #[test]
    fn kind_defaults_fill_gaps() {
        assert_eq!(
            pairs(&detect(&files(&[]), ProjectKind::Flutter)),
            [("flutter", "latest")]
        );
        assert_eq!(
            pairs(&detect(
                &files(&[(".nvmrc", "20")]),
                ProjectKind::ReactNative
            )),
            [("node", "20"), ("java", "temurin-17")]
        );
        assert_eq!(
            pairs(&detect(
                &files(&[("package.json", "{}")]),
                ProjectKind::General
            )),
            [("node", "lts")]
        );
        assert!(detect(&files(&[]), ProjectKind::General).is_empty());
    }

    #[test]
    fn validate_rejects_injection_and_unknown_tools() {
        let ok = |tool: &str, version: &str| ToolRequirement {
            tool: tool.into(),
            version: version.into(),
            source: ToolSource::User,
        };
        assert!(validate(&[ok("node", "20"), ok("java", "temurin-17.0.9+9")]).is_ok());
        assert!(validate(&[ok("node", "20; rm -rf /")]).is_err());
        assert!(validate(&[ok("node", "20\"\n- evil")]).is_err());
        assert!(validate(&[ok("curl-pipe-sh", "1")]).is_err());
        assert!(validate(&[ok("node", "")]).is_err());
    }
}
