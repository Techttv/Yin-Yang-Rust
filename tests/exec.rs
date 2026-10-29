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
