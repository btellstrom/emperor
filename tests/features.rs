//! Runs the approved Gherkin scenarios in `features/` against the emperor binary.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cucumber::gherkin::Step;
use cucumber::{World, given, then, when};
use tempfile::TempDir;

#[derive(Debug, World)]
struct ProjectWorld {
    project: TempDir,
    /// Document prefix -> document directory.
    documents: BTreeMap<String, PathBuf>,
    items: BTreeMap<String, ItemFixture>,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    /// The lockfile as it was right after the last Given step that accepted something.
    accepted_lockfile: Option<String>,
    /// The lockfile as it was after each command run by a When step.
    lockfile_after_runs: Vec<Option<String>>,
}

impl Default for ProjectWorld {
    fn default() -> Self {
        ProjectWorld {
            project: TempDir::new().expect("create temporary project"),
            documents: BTreeMap::new(),
            items: BTreeMap::new(),
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            accepted_lockfile: None,
            lockfile_after_runs: Vec::new(),
        }
    }
}

/// A Doorstop item, rendered the way Doorstop v3.2 writes it.
#[derive(Debug, Clone)]
struct ItemFixture {
    text: String,
    active: bool,
    links: Vec<String>,
    references: Vec<String>,
    justified_by: Vec<JustifiedBy>,
}

/// An entry of the custom `justified_by` attribute.
#[derive(Debug, Clone)]
struct JustifiedBy {
    kind: String,
    item: String,
    path: String,
}

impl ItemFixture {
    fn new(text: &str) -> ItemFixture {
        ItemFixture {
            text: text.to_string(),
            active: true,
            links: Vec::new(),
            references: Vec::new(),
            justified_by: Vec::new(),
        }
    }

    fn render(&self) -> String {
        let mut yaml = format!("active: {}\nderived: false\nheader: ''\n", self.active);
        if !self.justified_by.is_empty() {
            yaml += &yaml_list(
                "justified_by",
                self.justified_by.iter().map(|entry| {
                    format!(
                        "- item: {}\n  path: {}\n  type: {}\n",
                        entry.item, entry.path, entry.kind
                    )
                }),
            );
        }
        yaml += "level: 1.0\n";
        yaml += &yaml_list("links", self.links.iter().map(|uid| format!("- {uid}\n")));
        yaml += "normative: true\nref: ''\n";
        if !self.references.is_empty() {
            yaml += &yaml_list(
                "references",
                self.references
                    .iter()
                    .map(|path| format!("- path: {path}\n  type: file\n")),
            );
        }
        yaml + &format!("reviewed: null\ntext: |\n  {}\n", self.text)
    }
}

fn yaml_list(key: &str, entries: impl Iterator<Item = String>) -> String {
    let entries: String = entries.collect();
    if entries.is_empty() {
        format!("{key}: []\n")
    } else {
        format!("{key}:\n{entries}")
    }
}

impl ProjectWorld {
    fn path(&self, relative: &str) -> PathBuf {
        self.project.path().join(relative)
    }

    fn emperor(&self, args: &[&str]) -> std::process::Output {
        emperor_in(self.project.path(), args)
    }

    /// Runs a command line such as `emperor check` in `dir` and records its outcome.
    fn run_in(&mut self, dir: &Path, command: &str) {
        let mut words = command.split_whitespace();
        assert_eq!(
            words.next(),
            Some("emperor"),
            "only emperor commands are supported"
        );
        let args: Vec<&str> = words.collect();
        let output = emperor_in(dir, &args);
        self.record(output);
    }

    fn run(&mut self, command: &str) {
        let project = self.project.path().to_path_buf();
        self.run_in(&project, command);
    }

    fn record(&mut self, output: std::process::Output) {
        self.exit_code = output.status.code();
        self.stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        self.stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    }

    fn lockfile(&self) -> Option<String> {
        fs::read_to_string(self.path("emperor.lock")).ok()
    }

    fn item_path(&self, uid: &str) -> PathBuf {
        let directory = self
            .documents
            .iter()
            .filter(|(prefix, _)| uid.starts_with(prefix.as_str()))
            .max_by_key(|(prefix, _)| prefix.len())
            .map(|(_, dir)| dir)
            .unwrap_or_else(|| panic!("no document for {uid}"));
        directory.join(format!("{uid}.yml"))
    }

    fn write_item(&self, uid: &str) {
        fs::write(self.item_path(uid), self.items[uid].render()).unwrap();
    }

    fn add_item(&mut self, uid: &str, item: ItemFixture) {
        self.items.insert(uid.to_string(), item);
        self.write_item(uid);
    }

    fn update_item(&mut self, uid: &str, change: impl FnOnce(&mut ItemFixture)) {
        change(
            self.items
                .get_mut(uid)
                .unwrap_or_else(|| panic!("no requirement {uid}")),
        );
        self.write_item(uid);
    }

    fn write_file(&self, relative: &str, contents: &str) {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    /// Creates a justification file with placeholder contents unless a step already wrote it.
    fn ensure_file(&self, relative: &str) {
        if !self.path(relative).exists() {
            self.write_file(relative, &format!("contents of {relative}\n"));
        }
    }
}

fn emperor_in(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_emperor"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run emperor")
}

fn create_document(
    world: &mut ProjectWorld,
    prefix: String,
    parent: Option<&str>,
    item_format: &str,
) {
    let dir = world.path(&prefix.to_lowercase());
    fs::create_dir_all(&dir).unwrap();
    let parent = parent
        .map(|parent| format!("  parent: {parent}\n"))
        .unwrap_or_default();
    fs::write(
        dir.join(".doorstop.yml"),
        format!(
            "settings:\n  digits: 3\n  itemformat: {item_format}\n{parent}  prefix: {prefix}\n  sep: ''\n"
        ),
    )
    .unwrap();
    world.documents.insert(prefix, dir);
}

// Doorstop documents and items

#[given(expr = "a Doorstop document {string} with no parent")]
fn document_without_parent(world: &mut ProjectWorld, prefix: String) {
    create_document(world, prefix, None, "yaml");
}

#[given(expr = "a Doorstop document {string} with parent {string}")]
fn document_with_parent(world: &mut ProjectWorld, prefix: String, parent: String) {
    create_document(world, prefix, Some(&parent), "yaml");
}

#[given(expr = "the document {string} includes {string} in its reviewed attributes")]
fn document_reviews_attribute(world: &mut ProjectWorld, prefix: String, attribute: String) {
    let config = world.documents[&prefix].join(".doorstop.yml");
    let settings = fs::read_to_string(&config).unwrap();
    let attributes = format!("attributes:\n  reviewed:\n  - {attribute}\n");
    fs::write(config, settings + &attributes).unwrap();
}

#[given("an empty project")]
fn empty_project(_world: &mut ProjectWorld) {}

#[given(expr = "a Doorstop document {string} with no parent and item format {string}")]
fn document_with_item_format(world: &mut ProjectWorld, prefix: String, item_format: String) {
    create_document(world, prefix, None, &item_format);
}

#[given(expr = "requirement {string} with text {string} referencing file {string}")]
fn requirement_with_text_referencing_file(
    world: &mut ProjectWorld,
    uid: String,
    text: String,
    path: String,
) {
    world.ensure_file(&path);
    let mut item = ItemFixture::new(&text);
    item.references.push(path);
    world.add_item(&uid, item);
}

#[given(expr = "requirement {string} referencing file {string}")]
fn requirement_referencing_file(world: &mut ProjectWorld, uid: String, path: String) {
    world.ensure_file(&path);
    let mut item = ItemFixture::new(&uid);
    item.references.push(path);
    world.add_item(&uid, item);
}

#[given(expr = "requirement {string} justified by Rust function {string} in {string}")]
fn requirement_justified_by_rust_function(
    world: &mut ProjectWorld,
    uid: String,
    function: String,
    path: String,
) {
    requirement_justified_by(world, uid, "rust-fn".to_string(), function, path);
}

#[given(expr = "requirement {string} justified by {string} {string} in {string}")]
fn requirement_justified_by(
    world: &mut ProjectWorld,
    uid: String,
    kind: String,
    item: String,
    path: String,
) {
    let mut fixture = ItemFixture::new(&uid);
    fixture.justified_by.push(JustifiedBy { kind, item, path });
    world.add_item(&uid, fixture);
}

#[given(expr = "requirement {string} is also justified by Rust function {string} in {string}")]
fn requirement_also_justified_by(
    world: &mut ProjectWorld,
    uid: String,
    function: String,
    path: String,
) {
    world.update_item(&uid, |item| {
        item.justified_by.push(JustifiedBy {
            kind: "rust-fn".to_string(),
            item: function,
            path,
        })
    });
}

#[given(expr = "requirement {string} without justifications")]
fn requirement_without_justifications(world: &mut ProjectWorld, uid: String) {
    world.add_item(&uid, ItemFixture::new(&uid));
}

#[given(expr = "the reference to {string} is removed from {string}")]
fn reference_removed(world: &mut ProjectWorld, path: String, uid: String) {
    world.update_item(&uid, |item| {
        item.references.retain(|reference| *reference != path)
    });
}

#[given(expr = "requirement {string} is linked to {string} and {string}")]
fn requirement_linked_to_two(world: &mut ProjectWorld, uid: String, first: String, second: String) {
    world.update_item(&uid, |item| item.links.extend([first, second]));
}

#[given(expr = "requirement {string} is inactive")]
fn requirement_is_inactive(world: &mut ProjectWorld, uid: String) {
    world.update_item(&uid, |item| item.active = false);
}

#[given(expr = "requirement {string} is deleted")]
#[when(expr = "requirement {string} is deleted")]
fn requirement_is_deleted(world: &mut ProjectWorld, uid: String) {
    fs::remove_file(world.item_path(&uid)).unwrap();
    world.items.remove(&uid);
}

// Files

#[given(expr = "the file {string} contains:")]
#[when(expr = "the file {string} contains:")]
fn file_contains(world: &mut ProjectWorld, path: String, step: &Step) {
    let contents = step.docstring.as_ref().expect("the step has a doc string");
    world.write_file(&path, contents.strip_prefix('\n').unwrap_or(contents));
}

#[given(expr = "{string} is modified")]
#[when(expr = "{string} is modified")]
fn file_is_modified(world: &mut ProjectWorld, path: String) {
    let contents = fs::read_to_string(world.path(&path)).unwrap();
    world.write_file(&path, &format!("{contents}// modified\n"));
}

#[given(expr = "{string} is deleted")]
#[when(expr = "{string} is deleted")]
fn file_is_deleted(world: &mut ProjectWorld, path: String) {
    fs::remove_file(world.path(&path)).unwrap();
}

#[given(expr = "in {string} {string} is replaced with {string}")]
#[when(expr = "in {string} {string} is replaced with {string}")]
fn text_replaced(world: &mut ProjectWorld, path: String, from: String, to: String) {
    let (from, to) = (unescape(&from), unescape(&to));
    let contents = fs::read_to_string(world.path(&path)).unwrap();
    assert!(contents.contains(&from), "{path} does not contain {from:?}");
    world.write_file(&path, &contents.replace(&from, &to));
}

#[when(expr = "{string} is reformatted with all whitespace inside {string} doubled")]
fn whitespace_doubled(world: &mut ProjectWorld, path: String, function: String) {
    let contents = fs::read_to_string(world.path(&path)).unwrap();
    let (start, end) = function_span(&contents, &function);
    let doubled = contents[start..end]
        .replace(' ', "  ")
        .replace('\n', "\n\n");
    world.write_file(
        &path,
        &format!("{}{doubled}{}", &contents[..start], &contents[end..]),
    );
}

#[when(expr = "the comment {string} is added inside {string} in {string}")]
fn comment_added(world: &mut ProjectWorld, comment: String, function: String, path: String) {
    let contents = fs::read_to_string(world.path(&path)).unwrap();
    let (start, _) = function_span(&contents, &function);
    let body = start + contents[start..].find('\n').unwrap() + 1;
    let commented = format!("{}    {comment}\n{}", &contents[..body], &contents[body..]);
    world.write_file(&path, &commented);
}

/// Byte range from `fn <name>` to the end of the function, assuming it closes with "\n}".
fn function_span(contents: &str, function: &str) -> (usize, usize) {
    let start = contents
        .find(&format!("fn {function}"))
        .expect("function exists");
    let end = start + contents[start..].find("\n}").expect("function ends") + 2;
    (start, end)
}

/// Gherkin strings write newlines and quotes as `\n` and `\"`.
fn unescape(text: &str) -> String {
    text.replace("\\n", "\n").replace("\\\"", "\"")
}

#[when(expr = "a trailing newline is appended to {string}")]
fn trailing_newline_appended(world: &mut ProjectWorld, path: String) {
    let contents = fs::read_to_string(world.path(&path)).unwrap();
    world.write_file(&path, &format!("{contents}\n"));
}

// Test results

/// A JUnit report shaped like the one `cargo nextest` writes.
fn junit_report(testcases: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites name=\"nextest-run\">\n  \
         <testsuite name=\"emperor::login\">\n{testcases}  </testsuite>\n</testsuites>\n"
    )
}

fn junit_testcase(test: &str, outcome: &str) -> String {
    format!("    <testcase name=\"{test}\" classname=\"emperor::login\">{outcome}</testcase>\n")
}

#[given(expr = "the JUnit report {string} says {string} passed")]
fn junit_passed(world: &mut ProjectWorld, report: String, test: String) {
    world.write_file(&report, &junit_report(&junit_testcase(&test, "")));
}

#[given(expr = "the JUnit report {string} says {string} failed")]
fn junit_failed(world: &mut ProjectWorld, report: String, test: String) {
    let failure = "<failure message=\"assertion failed\" type=\"test failure\"/>";
    world.write_file(&report, &junit_report(&junit_testcase(&test, failure)));
}

#[given(expr = "the JUnit report {string} says {string} was skipped")]
fn junit_skipped(world: &mut ProjectWorld, report: String, test: String) {
    world.write_file(&report, &junit_report(&junit_testcase(&test, "<skipped/>")));
}

#[given(expr = "the JUnit report {string} contains no tests")]
fn junit_empty(world: &mut ProjectWorld, report: String) {
    world.write_file(&report, &junit_report(""));
}

// Judges

/// Configures a judge script that records its input and calls, then runs `body`.
fn configure_judge(world: &mut ProjectWorld, body: &str) {
    let script =
        format!("#!/bin/sh\ncat > judge-input.json\necho called >> judge-calls.log\n{body}\n");
    world.write_file("judge.sh", &script);
    world.write_file(
        "emperor.toml",
        "[judge]\ncommand = [\"sh\", \"judge.sh\"]\n",
    );
}

#[given(expr = "a judge that answers {string} with reason {string}")]
fn judge_answers(world: &mut ProjectWorld, verdict: String, reason: String) {
    let answer = format!("{{\"verdict\": \"{verdict}\", \"reason\": \"{reason}\"}}");
    configure_judge(world, &format!("printf '%s\\n' '{answer}'"));
}

#[given(expr = "a judge that exits with status {int}")]
fn judge_exits_with(world: &mut ProjectWorld, status: i32) {
    configure_judge(world, &format!("exit {status}"));
}

#[then("the judge received:")]
fn judge_received(world: &mut ProjectWorld, step: &Step) {
    let mut expected = step.docstring.clone().expect("the step has a doc string");
    for path in world.items.values().flat_map(|item| &item.references) {
        let contents = fs::read_to_string(world.path(path)).unwrap();
        let placeholder = format!("\"<contents of {path}>\"");
        expected = expected.replace(&placeholder, &serde_json::to_string(&contents).unwrap());
    }
    let expected: serde_json::Value = serde_json::from_str(&expected).expect("expected JSON");
    let received = fs::read_to_string(world.path("judge-input.json")).expect("the judge ran");
    let received: serde_json::Value = serde_json::from_str(&received).expect("the judge got JSON");
    assert_eq!(received, expected);
}

#[then(expr = "the judge was called {int} time(s)")]
fn judge_called(world: &mut ProjectWorld, expected: usize) {
    let calls = fs::read_to_string(world.path("judge-calls.log")).unwrap_or_default();
    assert_eq!(calls.lines().count(), expected);
}

// Running emperor

#[given(expr = "the justifications of {string} have been accepted")]
fn justifications_accepted(world: &mut ProjectWorld, uid: String) {
    let output = world.emperor(&["accept", &uid]);
    assert!(
        output.status.success(),
        "emperor accept failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    world.accepted_lockfile = world.lockfile();
}

#[when(expr = "I run {string} from another folder")]
fn run_from_another_folder(world: &mut ProjectWorld, command: String) {
    let project = world.project.path().display().to_string();
    let elsewhere = TempDir::new().unwrap();
    world.run_in(elsewhere.path(), &command.replace("<project>", &project));
}

#[given(expr = "I have run {string}")]
fn have_run(world: &mut ProjectWorld, command: String) {
    world.run(&command);
}

#[when(expr = "I run {string}")]
fn run(world: &mut ProjectWorld, command: String) {
    world.run(&command);
    let lockfile = world.lockfile();
    world.lockfile_after_runs.push(lockfile);
}

// Outcomes

#[then(expr = "the exit code is {int}")]
fn exit_code_is(world: &mut ProjectWorld, expected: i32) {
    assert_eq!(
        world.exit_code,
        Some(expected),
        "stdout: {:?}\nstderr: {:?}",
        world.stdout,
        world.stderr
    );
}

#[then(expr = "the output contains {string}")]
fn output_contains(world: &mut ProjectWorld, expected: String) {
    let expected = unescape(&expected);
    assert!(
        world.stdout.contains(&expected),
        "expected output to contain {expected:?}, got {:?}",
        world.stdout
    );
}

#[then("the output is:")]
fn output_is(world: &mut ProjectWorld, step: &Step) {
    let expected = step.docstring.as_ref().expect("the step has a doc string");
    assert_eq!(world.stdout.trim_end(), expected.trim());
}

#[then("the output is JSON equal to:")]
fn output_is_json(world: &mut ProjectWorld, step: &Step) {
    let expected = step.docstring.as_ref().expect("the step has a doc string");
    let expected: serde_json::Value = serde_json::from_str(expected).expect("expected JSON");
    let actual: serde_json::Value = serde_json::from_str(&world.stdout)
        .unwrap_or_else(|error| panic!("{error}: {:?}", world.stdout));
    assert_eq!(actual, expected);
}

#[then(expr = "the output contains {string} exactly once")]
fn output_contains_once(world: &mut ProjectWorld, expected: String) {
    let expected = unescape(&expected);
    assert_eq!(
        world.stdout.matches(&expected).count(),
        1,
        "{:?}",
        world.stdout
    );
}

#[then(expr = "the output does not contain {string}")]
fn output_lacks(world: &mut ProjectWorld, unexpected: String) {
    let unexpected = unescape(&unexpected);
    assert!(
        !world.stdout.contains(&unexpected),
        "expected output not to contain {unexpected:?}, got {:?}",
        world.stdout
    );
}

#[then(expr = "the error output contains {string}")]
fn error_output_contains(world: &mut ProjectWorld, expected: String) {
    let expected = unescape(&expected);
    assert!(
        world.stderr.contains(&expected),
        "expected error output to contain {expected:?}, got {:?}",
        world.stderr
    );
}

#[then(expr = "running {string} exits with {int}")]
fn running_exits_with(world: &mut ProjectWorld, command: String, expected: i32) {
    world.run(&command);
    exit_code_is(world, expected);
}

#[then("the lockfile is unchanged")]
fn lockfile_is_unchanged(world: &mut ProjectWorld) {
    assert_eq!(world.lockfile(), world.accepted_lockfile);
}

#[then("the lockfile is unchanged since the first accept")]
fn lockfile_unchanged_since_first_run(world: &mut ProjectWorld) {
    let first = world
        .lockfile_after_runs
        .first()
        .expect("a command was run");
    assert_eq!(world.lockfile().as_ref(), first.as_ref());
}

#[then("no lockfile exists")]
fn no_lockfile_exists(world: &mut ProjectWorld) {
    assert_eq!(world.lockfile(), None);
}

#[then(expr = "the lockfile contains an entry for {string}")]
fn lockfile_contains_entry(world: &mut ProjectWorld, uid: String) {
    assert!(
        lockfile_section_line(world, &uid).is_some(),
        "{:?}",
        world.lockfile()
    );
}

#[then(expr = "the lockfile does not contain an entry for {string}")]
fn lockfile_lacks_entry(world: &mut ProjectWorld, uid: String) {
    assert!(
        lockfile_section_line(world, &uid).is_none(),
        "{:?}",
        world.lockfile()
    );
}

#[then(expr = "{string} appears before {string} in the lockfile")]
fn appears_before_in_lockfile(world: &mut ProjectWorld, first: String, second: String) {
    let first = lockfile_section_line(world, &first).expect("first entry");
    let second = lockfile_section_line(world, &second).expect("second entry");
    assert!(first < second);
}

/// The line number of a requirement's section in the lockfile.
fn lockfile_section_line(world: &ProjectWorld, uid: &str) -> Option<usize> {
    let lockfile = world.lockfile().unwrap_or_default();
    lockfile.lines().position(|line| line == format!("[{uid}]"))
}

fn main() {
    // Approved scenarios that are not implemented yet are tagged @pending.
    futures::executor::block_on(
        ProjectWorld::cucumber()
            .fail_on_skipped()
            .filter_run_and_exit("features", |_, _, scenario| {
                !scenario.tags.iter().any(|tag| tag == "pending")
            }),
    );
}
