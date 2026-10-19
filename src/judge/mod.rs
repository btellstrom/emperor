//! Asks an external judge, such as an LLM, whether a requirement's justifications fulfil it.
//!
//! The judge is any command configured in `emperor.toml`. It receives a [`Request`] as JSON
//! on stdin and answers with a [`Verdict`] as JSON on stdout. Verdicts are cached until the
//! request or the command changes, so unchanged requirements are not judged again.

mod cache;
mod request;

use std::fmt;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::files::read_source;
use cache::{VerdictCache, cache_key};
pub use request::{Request, request};

const CONFIG_FILE: &str = "emperor.toml";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct JudgeConfig {
    /// The program and its arguments.
    pub command: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fulfilment {
    Fulfilled,
    Unfulfilled,
}

/// The judge's answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    pub verdict: Fulfilment,
    pub reason: String,
}

/// A verdict on a requirement, and whether it came from the cache.
#[derive(Debug, PartialEq, Eq)]
pub struct Judgement {
    pub requirement: String,
    pub verdict: Verdict,
    pub cached: bool,
}

impl fmt::Display for Fulfilment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Fulfilment::Fulfilled => write!(f, "fulfilled"),
            Fulfilment::Unfulfilled => write!(f, "unfulfilled"),
        }
    }
}

impl fmt::Display for Judgement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Verdict { verdict, reason } = &self.verdict;
        write!(f, "{}: {verdict} ({reason})", self.requirement)?;
        if self.cached {
            write!(f, " [cached]")?;
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    judge: Option<JudgeConfig>,
}

/// The judge configured in the project's `emperor.toml`.
pub fn load_config(root: &Path) -> Result<JudgeConfig> {
    let path = root.join(CONFIG_FILE);
    let Some(text) = read_source(&path)? else {
        return Err(Error::NoJudge);
    };
    let file: ConfigFile = toml::from_str(&text).map_err(Error::parse(&path))?;
    file.judge.ok_or(Error::NoJudge)
}

/// The judge's verdict on a request, from the cache when the request was judged before.
pub fn judge(root: &Path, config: &JudgeConfig, uid: &str, request: &Request) -> Result<Judgement> {
    let input = serde_json::to_string_pretty(request).expect("requests always serialise to JSON");
    let key = cache_key(config, &input);
    let mut cache = VerdictCache::load(root)?;
    let (verdict, cached) = match cache.verdicts.get(&key) {
        Some(verdict) => (verdict.clone(), true),
        None => {
            let verdict = ask(root, config, uid, &input)?;
            cache.verdicts.insert(key, verdict.clone());
            cache.save()?;
            (verdict, false)
        }
    };
    Ok(Judgement {
        requirement: uid.to_string(),
        verdict,
        cached,
    })
}

/// Runs the judge command with the request on stdin and reads its verdict from stdout.
fn ask(root: &Path, config: &JudgeConfig, uid: &str, input: &str) -> Result<Verdict> {
    let failed = |reason: String| Error::JudgeFailed {
        requirement: uid.to_string(),
        reason,
    };
    let (program, arguments) = config
        .command
        .split_first()
        .ok_or_else(|| failed("the judge command is empty".to_string()))?;
    let mut child = Command::new(program)
        .args(arguments)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| failed(format!("cannot run {program}: {error}")))?;
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let output = thread::scope(|scope| {
        // Written on its own thread so a judge that answers before reading cannot deadlock.
        // A judge that does not read its input is judged by its exit status and answer.
        scope.spawn(move || stdin.write_all(input.as_bytes()));
        child.wait_with_output()
    })
    .map_err(|error| failed(error.to_string()))?;
    if !output.status.success() {
        return Err(failed(output.status.to_string()));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| failed(format!("invalid verdict: {error}")))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::evidence;
    use crate::test_support::Project;

    const FULFILLED: &str = r#"{"verdict": "fulfilled", "reason": "test covers it"}"#;

    /// A judge that saves its input, counts its calls and prints `answer`.
    fn scripted_judge(answer: &str) -> JudgeConfig {
        let script = format!("cat > judge-input.json; echo called >> calls.log; echo '{answer}'");
        JudgeConfig {
            command: vec!["sh".into(), "-c".into(), script],
        }
    }

    fn login_project() -> Project {
        Project::new()
            .requirement("REQ001", &["login.md"])
            .text("REQ001", "Login shall reject a wrong password")
    }

    fn request_for(project: &Project, uid: &str) -> Request {
        let tree = project.tree();
        request(&tree, &evidence::gather(&tree).unwrap(), uid).unwrap()
    }

    fn calls(project: &Project) -> usize {
        fs::read_to_string(project.path("calls.log"))
            .unwrap_or_default()
            .lines()
            .count()
    }

    #[test]
    fn reads_the_judge_from_emperor_toml() {
        let project = Project::new().file(
            "emperor.toml",
            "[judge]\ncommand = [\"llm-judge\", \"--strict\"]\n",
        );
        assert_eq!(
            load_config(&project.path("")).unwrap(),
            JudgeConfig {
                command: vec!["llm-judge".into(), "--strict".into()]
            }
        );
    }

    #[test]
    fn without_a_judge_section_there_is_no_judge() {
        assert!(matches!(
            load_config(&Project::new().path("")),
            Err(Error::NoJudge)
        ));
        let other = Project::new().file("emperor.toml", "[other]\n");
        assert!(matches!(load_config(&other.path("")), Err(Error::NoJudge)));
    }

    #[test]
    fn the_request_holds_the_requirement_and_the_content_of_its_justifications() {
        let request = serde_json::to_value(request_for(&login_project(), "REQ001")).unwrap();
        assert_eq!(
            request,
            serde_json::json!({
                "requirement": { "id": "REQ001", "text": "Login shall reject a wrong password" },
                "justifications": [
                    { "type": "file", "path": "login.md", "content": "contents of login.md" }
                ]
            })
        );
    }

    #[test]
    fn the_request_holds_the_source_of_a_rust_function() {
        let mut project = login_project().file(
            "tests/login.rs",
            "#[test]\nfn rejects() {}\n\nfn other() {}\n",
        );
        project.justify_by_rust_fn("REQ001", "tests/login.rs", "rejects");
        let request = serde_json::to_value(request_for(&project, "REQ001")).unwrap();
        assert_eq!(
            request["justifications"][1]["content"],
            "#[test]\nfn rejects() {}"
        );
    }

    #[test]
    fn the_judge_gets_the_request_on_stdin_and_its_verdict_is_returned() {
        let project = login_project();
        let request = request_for(&project, "REQ001");
        let judgement = judge(
            &project.path(""),
            &scripted_judge(FULFILLED),
            "REQ001",
            &request,
        )
        .unwrap();
        assert_eq!(
            judgement,
            Judgement {
                requirement: "REQ001".into(),
                verdict: Verdict {
                    verdict: Fulfilment::Fulfilled,
                    reason: "test covers it".into()
                },
                cached: false
            }
        );
        let received: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(project.path("judge-input.json")).unwrap())
                .unwrap();
        assert_eq!(received, serde_json::to_value(&request).unwrap());
    }

    #[test]
    fn a_failing_judge_is_an_error() {
        let project = login_project();
        let failing = JudgeConfig {
            command: vec!["sh".into(), "-c".into(), "exit 3".into()],
        };
        let error = judge(
            &project.path(""),
            &failing,
            "REQ001",
            &request_for(&project, "REQ001"),
        )
        .unwrap_err();
        assert!(
            error.to_string().starts_with("judge failed for REQ001"),
            "{error}"
        );
    }

    #[test]
    fn an_answer_that_is_not_a_verdict_is_an_error() {
        let project = login_project();
        let error = judge(
            &project.path(""),
            &scripted_judge("looks fine to me"),
            "REQ001",
            &request_for(&project, "REQ001"),
        )
        .unwrap_err();
        assert!(matches!(error, Error::JudgeFailed { .. }));
    }

    #[test]
    fn an_unchanged_request_is_answered_from_the_cache() {
        let project = login_project();
        let request = request_for(&project, "REQ001");
        judge(
            &project.path(""),
            &scripted_judge(FULFILLED),
            "REQ001",
            &request,
        )
        .unwrap();
        let again = judge(
            &project.path(""),
            &scripted_judge(FULFILLED),
            "REQ001",
            &request,
        )
        .unwrap();
        assert!(again.cached);
        assert_eq!(calls(&project), 1);
    }

    #[test]
    fn a_changed_request_is_judged_again() {
        let project = login_project();
        judge(
            &project.path(""),
            &scripted_judge(FULFILLED),
            "REQ001",
            &request_for(&project, "REQ001"),
        )
        .unwrap();
        fs::write(project.path("login.md"), "changed").unwrap();
        let again = judge(
            &project.path(""),
            &scripted_judge(FULFILLED),
            "REQ001",
            &request_for(&project, "REQ001"),
        )
        .unwrap();
        assert!(!again.cached);
        assert_eq!(calls(&project), 2);
    }

    #[test]
    fn judgements_are_described_with_their_reason() {
        let judgement = Judgement {
            requirement: "REQ001".into(),
            verdict: Verdict {
                verdict: Fulfilment::Unfulfilled,
                reason: "no assertion".into(),
            },
            cached: true,
        };
        assert_eq!(
            judgement.to_string(),
            "REQ001: unfulfilled (no assertion) [cached]"
        );
    }
}
