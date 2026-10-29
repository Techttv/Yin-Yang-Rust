use std::time::{Duration, Instant};

use yinyang::exec::{Exec, RealExec};

#[test]
fn run_captures_stdout_and_status() {
    let out = RealExec::new().run("sh", &["-c", "printf hello; exit 3"]).unwrap();
    assert_eq!(out.stdout, "hello");
    assert_eq!(out.status, 3);
}

#[test]
fn run_of_a_missing_program_is_an_error() {
    assert!(RealExec::new().run("definitely-not-a-program-yinyang", &[]).is_err());
}

#[test]
fn exists_searches_the_path_and_accepts_explicit_paths() {
    assert!(RealExec::new().exists("sh"));
    assert!(RealExec::new().exists("/bin/sh"));
    assert!(!RealExec::new().exists("definitely-not-a-program-yinyang"));
    assert!(!RealExec::new().exists("/nonexistent/program"));
}

#[test]
fn run_inherited_returns_the_exit_status() {
    assert_eq!(RealExec::new().run_inherited("sh", &["-c", "exit 4"]).unwrap(), 4);
    assert_eq!(RealExec::new().run_inherited("sh", &["-c", "true"]).unwrap(), 0);
    assert!(RealExec::new().run_inherited("definitely-not-a-program-yinyang", &[]).is_err());
}

#[test]
fn run_inherited_does_not_wait_for_background_children() {
    // A captured pipe would stay open until the background process ends, blocking for two seconds.
    let started = std::time::Instant::now();
    assert_eq!(RealExec::new().run_inherited("sh", &["-c", "sleep 2 &"]).unwrap(), 0);
    assert!(started.elapsed() < std::time::Duration::from_millis(1500), "waited {:?}", started.elapsed());
}

#[test]
fn run_captures_stderr_and_check_reports_it() {
    let out = RealExec::new().run("sh", &["-c", "echo oops >&2; exit 2"]).unwrap();
    assert_eq!(out.stderr, "oops\n");
    let err = out.check("my-tool").unwrap_err().to_string();
    assert!(err.contains("my-tool") && err.contains("status 2") && err.contains("oops"), "{err}");
}

#[test]
fn check_passes_a_successful_output_through() {
    let out = RealExec::new().run("sh", &["-c", "printf fine"]).unwrap().check("sh").unwrap();
    assert_eq!(out.stdout, "fine");
}

#[test]
fn check_without_stderr_still_names_the_status() {
    let err = RealExec::new().run("sh", &["-c", "exit 7"]).unwrap().check("sh").unwrap_err().to_string();
    assert_eq!(err, "sh exited with status 7");
}

#[test]
fn a_program_that_does_not_finish_is_killed_and_reported() {
    let exec = RealExec::with_timeout(Duration::from_millis(300));
    let started = Instant::now();
    let err = exec.run("sleep", &["5"]).unwrap_err();
    assert!(started.elapsed() < Duration::from_secs(3), "took {:?}", started.elapsed());
    assert!(format!("{err:#}").contains("did not finish"), "{err:#}");

    let started = Instant::now();
    let err = exec.run_inherited("sleep", &["5"]).unwrap_err();
    assert!(started.elapsed() < Duration::from_secs(3), "took {:?}", started.elapsed());
    assert!(format!("{err:#}").contains("did not finish"), "{err:#}");
}

#[test]
fn a_quick_program_is_not_affected_by_the_timeout() {
    let exec = RealExec::with_timeout(Duration::from_secs(10));
    assert_eq!(exec.run("sh", &["-c", "exit 3"]).unwrap().status, 3);
    assert_eq!(exec.run_inherited("sh", &["-c", "exit 4"]).unwrap(), 4);
}

#[test]
fn output_larger_than_a_pipe_buffer_is_captured_whole() {
    let out = RealExec::new()
        .run("sh", &["-c", "head -c 1000000 /dev/zero | tr '\\0' x; head -c 300000 /dev/zero | tr '\\0' y >&2"])
        .unwrap();
    assert_eq!(out.stdout.len(), 1_000_000);
    assert_eq!(out.stderr.len(), 300_000);
}
