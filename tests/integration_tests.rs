use freo::{run, AppConfig, CommentTokenResolver};
use std::collections::HashMap;
use std::fs;
use tempfile::tempdir;

#[test]
fn run_removes_keyword_comments_and_skips_unknown_extensions() {
    let dir = tempdir().expect("temp dir");

    let rs_path = dir.path().join("example.rs");
    fs::write(
        &rs_path,
        "fn main() {}\n// FREO remove me\nlet x = 5; // keep\n",
    )
    .unwrap();

    let py_path = dir.path().join("script.py");
    fs::write(
        &py_path,
        "print('hi') # freo: remove this\nprint('stay') # note\n",
    )
    .unwrap();

    let unknown_path = dir.path().join("data.txt");
    let unknown_original = "value // FREO remove?";
    fs::write(&unknown_path, unknown_original).unwrap();

    let config = AppConfig::new(None, None);
    let resolver = CommentTokenResolver::new(config.comment_map().cloned());

    run(
        &config,
        &[rs_path.clone(), py_path.clone(), unknown_path.clone()],
        &resolver,
    )
    .expect("run should succeed");

    let rs_out = fs::read_to_string(&rs_path).unwrap();
    assert_eq!(rs_out, "fn main() {}\nlet x = 5; // keep\n");

    let py_out = fs::read_to_string(&py_path).unwrap();
    assert_eq!(py_out, "print('hi')\nprint('stay') # note\n");

    let unknown_out = fs::read_to_string(&unknown_path).unwrap();
    assert_eq!(unknown_out, unknown_original);
}

#[test]
fn run_respects_custom_comment_tokens_and_keyword_from_config() {
    let dir = tempdir().expect("temp dir");

    let file_path = dir.path().join("notes.cust");
    fs::write(
        &file_path,
        "keep ;; TODO other\nvalue ;; Ticket-123: remove this\nfinal line\n",
    )
    .unwrap();

    let custom_map = HashMap::from([("cust".to_string(), ";;".to_string())]);
    let config = AppConfig::new(Some("ticket-123".to_string()), Some(custom_map));
    let resolver = CommentTokenResolver::new(config.comment_map().cloned());

    run(&config, std::slice::from_ref(&file_path), &resolver).expect("run should succeed");

    let output = fs::read_to_string(&file_path).unwrap();
    assert_eq!(output, "keep ;; TODO other\nvalue\nfinal line\n");
}

#[test]
fn remove_matching_comments_preserves_file_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().expect("temp dir");
    let file_path = dir.path().join("script.rs");

    fs::write(
        &file_path,
        "fn main() {}\n// FREO remove me\nlet x = 5; // keep\n",
    )
    .unwrap();

    let original_mode = 0o654;
    let mut perms = fs::metadata(&file_path).unwrap().permissions();
    perms.set_mode(original_mode);
    fs::set_permissions(&file_path, perms).unwrap();

    let config = AppConfig::new(None, None);
    let resolver = CommentTokenResolver::new(config.comment_map().cloned());

    run(&config, std::slice::from_ref(&file_path), &resolver).expect("run should succeed");

    let output = fs::read_to_string(&file_path).unwrap();
    assert_eq!(output, "fn main() {}\nlet x = 5; // keep\n");

    let resulting_mode = fs::metadata(&file_path).unwrap().permissions().mode() & 0o777;
    assert_eq!(resulting_mode, original_mode);
}

#[test]
fn run_removes_block_marker_regions_across_languages() {
    let dir = tempdir().expect("temp dir");

    let rs_path = dir.path().join("example.rs");
    fs::write(
        &rs_path,
        "fn main() {\n// FREO-BEGIN\n// polling instead of a webhook: the vendor\n// has no webhook API yet\n// FREO-END\nlet x = 5;\n}\n",
    )
    .unwrap();

    let py_path = dir.path().join("script.py");
    fs::write(
        &py_path,
        "# freo-begin\n# reviewer note spanning\n# two lines\n# freo-end\nprint('stay')\n",
    )
    .unwrap();

    let config = AppConfig::new(None, None);
    let resolver = CommentTokenResolver::new(config.comment_map().cloned());

    run(&config, &[rs_path.clone(), py_path.clone()], &resolver).expect("run should succeed");

    assert_eq!(
        fs::read_to_string(&rs_path).unwrap(),
        "fn main() {\nlet x = 5;\n}\n"
    );
    assert_eq!(fs::read_to_string(&py_path).unwrap(), "print('stay')\n");
}

#[test]
fn run_leaves_the_file_untouched_when_a_block_marker_is_unterminated() {
    let dir = tempdir().expect("temp dir");

    let file_path = dir.path().join("example.rs");
    let original = "fn main() {}\n// FREO-BEGIN\n// note without an end marker\nlet x = 5;\n";
    fs::write(&file_path, original).unwrap();

    let config = AppConfig::new(None, None);
    let resolver = CommentTokenResolver::new(config.comment_map().cloned());

    let error = run(&config, std::slice::from_ref(&file_path), &resolver)
        .expect_err("unterminated block should fail the run");

    assert!(error.to_string().contains("example.rs"), "{error}");
    assert_eq!(
        fs::read_to_string(&file_path).unwrap(),
        original,
        "the file must be byte-identical after a refused run"
    );
}
