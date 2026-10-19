//! Runs the approved Gherkin scenarios in `features/` against the emperor binary.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

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
}

impl ItemFixture {
    fn new(text: &str) -> ItemFixture {
        ItemFixture {
            text: text.to_string(),
            active: true,
            links: Vec::new(),
            references: Vec::new(),
        }
    }

    fn render(&self) -> String {
        let mut yaml = format!(
            "active: {}\nderived: false\nheader: ''\nlevel: 1.0\n",
            self.active
        );
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
        Command::new(env!("CARGO_BIN_EXE_emperor"))
            .args(args)
            .current_dir(self.project.path())
            .output()
            .expect("run emperor")
    }

    fn run(&mut self, command: &str) {
        let mut words = command.split_whitespace();
        assert_eq!(
            words.next(),
            Some("emperor"),
            "only emperor commands are supported"
        );
        let args: Vec<&str> = words.collect();
        let output = self.emperor(&args);
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

fn create_document(world: &mut ProjectWorld, prefix: String, item_format: &str) {
    let dir = world.path(&prefix.to_lowercase());
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(".doorstop.yml"),
        format!(
            "settings:\n  digits: 3\n  itemformat: {item_format}\n  prefix: {prefix}\n  sep: ''\n"
        ),
    )
    .unwrap();
    world.documents.insert(prefix, dir);
}

// Doorstop documents and items

#[given(expr = "a Doorstop document {string} with no parent")]
fn document_without_parent(world: &mut ProjectWorld, prefix: String) {
    create_document(world, prefix, "yaml");
}

#[given(expr = "a Doorstop document {string} with no parent and item format {string}")]
fn document_with_item_format(world: &mut ProjectWorld, prefix: String, item_format: String) {
    create_document(world, prefix, &item_format);
}

#[given(expr = "requirement {string} referencing file {string}")]
fn requirement_referencing_file(world: &mut ProjectWorld, uid: String, path: String) {
    world.ensure_file(&path);
    let mut item = ItemFixture::new(&uid);
    item.references.push(path);
    world.add_item(&uid, item);
}

#[given(expr = "requirement {string} without justifications")]
fn requirement_without_justifications(world: &mut ProjectWorld, uid: String) {
    world.add_item(&uid, ItemFixture::new(&uid));
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

#[when(expr = "a trailing newline is appended to {string}")]
fn trailing_newline_appended(world: &mut ProjectWorld, path: String) {
    let contents = fs::read_to_string(world.path(&path)).unwrap();
    world.write_file(&path, &format!("{contents}\n"));
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

#[when(expr = "I run {string}")]
fn run(world: &mut ProjectWorld, command: String) {
    world.run(&command);
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
    assert!(
        world.stdout.contains(&expected),
        "expected output to contain {expected:?}, got {:?}",
        world.stdout
    );
}

#[then(expr = "the error output contains {string}")]
fn error_output_contains(world: &mut ProjectWorld, expected: String) {
    assert!(
        world.stderr.contains(&expected),
        "expected error output to contain {expected:?}, got {:?}",
        world.stderr
    );
}

#[then("the lockfile is unchanged")]
fn lockfile_is_unchanged(world: &mut ProjectWorld) {
    assert_eq!(world.lockfile(), world.accepted_lockfile);
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
