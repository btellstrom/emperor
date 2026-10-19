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
    exit_code: Option<i32>,
    stderr: String,
}

impl Default for ProjectWorld {
    fn default() -> Self {
        ProjectWorld {
            project: TempDir::new().expect("create temporary project"),
            documents: BTreeMap::new(),
            exit_code: None,
            stderr: String::new(),
        }
    }
}

impl ProjectWorld {
    fn emperor(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_emperor"))
            .args(args)
            .current_dir(self.project.path())
            .output()
            .expect("run emperor")
    }

    fn document_for(&self, uid: &str) -> &PathBuf {
        self.documents
            .iter()
            .filter(|(prefix, _)| uid.starts_with(prefix.as_str()))
            .max_by_key(|(prefix, _)| prefix.len())
            .map(|(_, dir)| dir)
            .unwrap_or_else(|| panic!("no document for {uid}"))
    }
}

// The files below mirror what Doorstop v3.2 writes.

fn create_document(world: &mut ProjectWorld, prefix: String, item_format: &str) {
    let dir = world.project.path().join(prefix.to_lowercase());
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(".doorstop.yml"),
        format!("settings:\n  digits: 3\n  itemformat: {item_format}\n  prefix: {prefix}\n  sep: ''\n"),
    )
    .unwrap();
    world.documents.insert(prefix, dir);
}

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
    let file = world.project.path().join(&path);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "#[test]\nfn rejects_bad_password() {}\n").unwrap();

    let item = format!(
        "active: true\nderived: false\nheader: ''\nlevel: 1.0\nlinks: []\nnormative: true\n\
         ref: ''\nreferences:\n- path: {path}\n  type: file\nreviewed: null\ntext: |\n  {uid}\n"
    );
    fs::write(world.document_for(&uid).join(format!("{uid}.yml")), item).unwrap();
}

#[given(expr = "the justifications of {string} have been accepted")]
fn justifications_accepted(world: &mut ProjectWorld, uid: String) {
    let output = world.emperor(&["accept", &uid]);
    assert!(
        output.status.success(),
        "emperor accept failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[when(expr = "I run {string}")]
fn run(world: &mut ProjectWorld, command: String) {
    let mut words = command.split_whitespace();
    assert_eq!(words.next(), Some("emperor"), "only emperor commands are supported");
    let args: Vec<&str> = words.collect();
    let output = world.emperor(&args);
    world.exit_code = output.status.code();
    world.stderr = String::from_utf8_lossy(&output.stderr).into_owned();
}

#[then(expr = "the exit code is {int}")]
fn exit_code_is(world: &mut ProjectWorld, expected: i32) {
    assert_eq!(world.exit_code, Some(expected));
}

#[then(expr = "the error output contains {string}")]
fn error_output_contains(world: &mut ProjectWorld, expected: String) {
    assert!(
        world.stderr.contains(&expected),
        "expected error output to contain {expected:?}, got {:?}",
        world.stderr
    );
}

fn main() {
    // Approved scenarios that are not implemented yet are tagged @pending.
    futures::executor::block_on(ProjectWorld::cucumber().fail_on_skipped().filter_run_and_exit(
        "features",
        |_, _, scenario| !scenario.tags.iter().any(|tag| tag == "pending"),
    ));
}
