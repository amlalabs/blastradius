//! Report writes should not follow attacker-controlled report-file symlinks in
//! the output directory.

use std::fs;
use std::process::Command;
use std::process::Stdio;

#[cfg(unix)]
#[test]
fn report_write_replaces_symlink_instead_of_following_it() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("out");
    fs::create_dir_all(&out).unwrap();

    let victim = tmp.path().join("victim.txt");
    fs::write(&victim, "do not clobber").unwrap();
    let report_path = out.join("blastradius-report.md");
    symlink(&victim, &report_path).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_blastradius"))
        .args([
            "scan",
            "--report",
            "--max-depth",
            "0",
            "--max-repos",
            "0",
            "--output",
        ])
        .arg(&out)
        .current_dir(tmp.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();

    assert!(status.success());
    assert_eq!(fs::read_to_string(&victim).unwrap(), "do not clobber");

    let meta = fs::symlink_metadata(&report_path).unwrap();
    assert!(
        !meta.file_type().is_symlink(),
        "report symlink was not replaced"
    );
    assert!(fs::read_to_string(&report_path)
        .unwrap()
        .contains("# blastradius report"));
}

#[cfg(unix)]
#[test]
fn output_directory_symlink_is_rejected() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("target");
    let out = tmp.path().join("out-link");
    fs::create_dir_all(&target).unwrap();
    symlink(&target, &out).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_blastradius"))
        .args([
            "scan",
            "--report",
            "--max-depth",
            "0",
            "--max-repos",
            "0",
            "--output",
        ])
        .arg(&out)
        .current_dir(tmp.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();

    assert!(!status.success());
    assert!(!target.join("blastradius-report.md").exists());
    assert!(!target.join("blastradius-report.json").exists());
}

#[test]
fn output_directory_alone_writes_both_report_formats() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("audit");

    let status = Command::new(env!("CARGO_BIN_EXE_blastradius"))
        .args([
            "scan",
            "--max-depth",
            "0",
            "--max-repos",
            "0",
            "--output",
        ])
        .arg(&out)
        .current_dir(tmp.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();

    assert!(status.success());
    assert!(out.join("blastradius-report.md").is_file());
    assert!(out.join("blastradius-report.json").is_file());
}

#[cfg(unix)]
#[test]
fn report_files_are_private_even_with_permissive_umask() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("audit");

    let status = Command::new("sh")
        .arg("-c")
        .arg("umask 000; exec \"$@\"")
        .arg("sh")
        .arg(env!("CARGO_BIN_EXE_blastradius"))
        .args([
            "scan",
            "--max-depth",
            "0",
            "--max-repos",
            "0",
            "--output",
        ])
        .arg(&out)
        .current_dir(tmp.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();

    assert!(status.success());
    for name in ["blastradius-report.md", "blastradius-report.json"] {
        let mode = fs::metadata(out.join(name)).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "{name} mode should be private");
    }
}
