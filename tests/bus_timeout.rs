// Alone in its own test binary because it changes the process environment.
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use yinyang::bus::{Bus, RealBus};

struct Slow;

#[zbus::interface(name = "org.yinyang.Slow")]
impl Slow {
    fn wait(&self, _arg: &str) {
        thread::sleep(Duration::from_secs(5));
    }
}

struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn a_service_that_never_answers_is_an_error_not_a_hang() {
    let mut child = Command::new("/usr/bin/dbus-daemon")
        .args(["--session", "--nofork", "--print-address=1"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("dbus-daemon is needed for this test");
    let mut address = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut address).unwrap();
    let _daemon = Daemon(child);
    // SAFETY: this is the only test in this binary, and no other thread exists yet.
    unsafe { std::env::set_var("DBUS_SESSION_BUS_ADDRESS", address.trim()) };

    let (ready, wait_ready) = mpsc::channel();
    thread::spawn(move || {
        let _connection = zbus::blocking::connection::Builder::session()
            .unwrap()
            .name("org.yinyang.Slow")
            .unwrap()
            .serve_at("/slow", Slow)
            .unwrap()
            .build()
            .unwrap();
        ready.send(()).unwrap();
        thread::sleep(Duration::from_secs(30));
    });
    wait_ready.recv_timeout(Duration::from_secs(10)).unwrap();

    let bus = RealBus::with_timeout(Duration::from_millis(300));
    let started = Instant::now();
    let result = bus.call_str("org.yinyang.Slow", "/slow", "org.yinyang.Slow", "Wait", "x");
    assert!(result.is_err());
    assert!(started.elapsed() < Duration::from_secs(3), "took {:?}", started.elapsed());
}
