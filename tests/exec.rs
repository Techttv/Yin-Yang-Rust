use yinyang::exec::{Exec, RealExec};

#[test]
fn run_captures_stdout_and_status() {
    let out = RealExec.run("sh", &["-c", "printf hello; exit 3"]).unwrap();
    assert_eq!(out.stdout, "hello");
    assert_eq!(out.status, 3);
}

#[test]
fn run_of_a_missing_program_is_an_error() {
    assert!(RealExec.run("definitely-not-a-program-yinyang", &[]).is_err());
}

#[test]
fn exists_searches_the_path_and_accepts_explicit_paths() {
    assert!(RealExec.exists("sh"));
    assert!(RealExec.exists("/bin/sh"));
    assert!(!RealExec.exists("definitely-not-a-program-yinyang"));
    assert!(!RealExec.exists("/nonexistent/program"));
}

#[test]
fn run_inherited_returns_the_exit_status() {
    assert_eq!(RealExec.run_inherited("sh", &["-c", "exit 4"]).unwrap(), 4);
    assert_eq!(RealExec.run_inherited("sh", &["-c", "true"]).unwrap(), 0);
    assert!(RealExec.run_inherited("definitely-not-a-program-yinyang", &[]).is_err());
}

#[test]
fn run_inherited_does_not_wait_for_background_children() {
    // A captured pipe would stay open until the background process ends, blocking for two seconds.
    let started = std::time::Instant::now();
    assert_eq!(RealExec.run_inherited("sh", &["-c", "sleep 2 &"]).unwrap(), 0);
    assert!(started.elapsed() < std::time::Duration::from_millis(1500), "waited {:?}", started.elapsed());
}
