use chrono::{FixedOffset, NaiveDate, NaiveTime, Timelike};
use tempfile::TempDir;
use yinyang::config::{Config, Mode};
use yinyang::location::{parse_ipinfo, resolve, LocationSource};
use yinyang::paths::Paths;
use yinyang::schedule::{effective_times, should_be_dark, sun_times};

fn t(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap()
}

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn secs(t: NaiveTime) -> i64 {
    i64::from(t.num_seconds_from_midnight())
}

/// Asserts `actual` is within two minutes of `h:m:s`.
fn near(actual: NaiveTime, h: u32, m: u32, s: u32) {
    let expected = NaiveTime::from_hms_opt(h, m, s).unwrap();
    assert!(
        (secs(actual) - secs(expected)).abs() <= 120,
        "{actual} is not within 2 minutes of {expected}"
    );
}

struct FakeLoc(Option<(f64, f64)>);

impl LocationSource for FakeLoc {
    fn current(&self) -> Option<(f64, f64)> {
        self.0
    }
}

fn config(mode: Mode, update_location: bool, coordinates: (f64, f64)) -> Config {
    Config {
        mode,
        update_location,
        coordinates,
        ..Config::default()
    }
}

#[test]
fn should_be_dark_during_the_day_cycle() {
    assert!(!should_be_dark(t(12, 0), t(7, 0), t(20, 0)));
    assert!(should_be_dark(t(21, 0), t(7, 0), t(20, 0)));
    assert!(should_be_dark(t(6, 0), t(7, 0), t(20, 0)));
}

#[test]
fn should_be_dark_boundaries_are_exact() {
    assert!(!should_be_dark(t(7, 0), t(7, 0), t(20, 0)), "light starts at 07:00");
    assert!(should_be_dark(t(20, 0), t(7, 0), t(20, 0)), "dark starts at 20:00");
}

#[test]
fn should_be_dark_when_light_period_crosses_midnight() {
    assert!(!should_be_dark(t(23, 0), t(22, 0), t(6, 0)));
    assert!(!should_be_dark(t(3, 0), t(22, 0), t(6, 0)));
    assert!(should_be_dark(t(12, 0), t(22, 0), t(6, 0)));
}

#[test]
fn should_be_dark_with_equal_times_is_deterministic() {
    let first = should_be_dark(t(12, 0), t(7, 0), t(7, 0));
    assert_eq!(first, should_be_dark(t(12, 0), t(7, 0), t(7, 0)));
}

#[test]
fn sun_times_rome_matches_reference_values() {
    let (rise, set) = sun_times(41.9028, 12.4964, date(2026, 6, 21)).unwrap();
    near(rise.time(), 3, 34, 49);
    near(set.time(), 18, 48, 38);
    let (rise, set) = sun_times(41.9028, 12.4964, date(2026, 12, 21)).unwrap();
    near(rise.time(), 6, 34, 16);
    near(set.time(), 15, 41, 40);
}

#[test]
fn sun_times_is_none_in_midnight_sun_and_polar_night() {
    assert!(sun_times(69.65, 18.96, date(2026, 6, 21)).is_none());
    assert!(sun_times(69.65, 18.96, date(2026, 12, 21)).is_none());
}

#[test]
fn effective_times_converts_to_local_time_across_utc_dates() {
    let home = TempDir::new().unwrap();
    let paths = Paths::under(home.path());
    let cfg = config(Mode::FollowSun, false, (-36.85, 174.76));
    let nz = FixedOffset::east_opt(13 * 3600).unwrap();
    let (rise, set) = effective_times(&cfg, &FakeLoc(None), &paths, date(2026, 10, 29), nz).unwrap();
    near(rise, 6, 19, 34); // the sunrise falls on the 28th in UTC
    near(set, 19, 49, 40);

    let cfg = config(Mode::FollowSun, false, (21.3, -157.86));
    let hawaii = FixedOffset::west_opt(10 * 3600).unwrap();
    let (rise, set) =
        effective_times(&cfg, &FakeLoc(None), &paths, date(2026, 10, 29), hawaii).unwrap();
    near(rise, 6, 32, 57);
    near(set, 17, 57, 9); // the sunset falls on the 30th in UTC
}

#[test]
fn effective_times_uses_manual_times_unless_following_the_sun() {
    let home = TempDir::new().unwrap();
    let paths = Paths::under(home.path());
    let utc = FixedOffset::east_opt(0).unwrap();
    for mode in [Mode::Manual, Mode::Scheduled] {
        let mut cfg = config(mode, true, (0.0, 0.0));
        cfg.times = (t(8, 15), t(19, 45));
        let got = effective_times(&cfg, &FakeLoc(None), &paths, date(2026, 6, 21), utc).unwrap();
        assert_eq!(got, (t(8, 15), t(19, 45)));
    }
}

#[test]
fn effective_times_falls_back_to_manual_times_in_polar_regions() {
    let home = TempDir::new().unwrap();
    let paths = Paths::under(home.path());
    let mut cfg = config(Mode::FollowSun, false, (69.65, 18.96));
    cfg.times = (t(8, 0), t(21, 0));
    let utc = FixedOffset::east_opt(0).unwrap();
    let got = effective_times(&cfg, &FakeLoc(None), &paths, date(2026, 6, 21), utc).unwrap();
    assert_eq!(got, (t(8, 0), t(21, 0)));
}

#[test]
fn effective_times_rejects_out_of_range_coordinates() {
    let home = TempDir::new().unwrap();
    let paths = Paths::under(home.path());
    let cfg = config(Mode::FollowSun, false, (91.0, 10.0));
    let utc = FixedOffset::east_opt(0).unwrap();
    assert!(effective_times(&cfg, &FakeLoc(None), &paths, date(2026, 6, 21), utc).is_err());
}

#[test]
fn resolve_without_source_or_saved_location_is_an_error() {
    let home = TempDir::new().unwrap();
    let paths = Paths::under(home.path());
    let cfg = config(Mode::FollowSun, true, (0.0, 0.0));
    assert!(resolve(&cfg, &paths, &FakeLoc(None)).is_err());
}

#[test]
fn resolve_falls_back_to_the_last_known_location() {
    let home = TempDir::new().unwrap();
    let paths = Paths::under(home.path());
    let cfg = config(Mode::FollowSun, true, (0.0, 0.0));
    assert_eq!(resolve(&cfg, &paths, &FakeLoc(Some((45.0, 9.0)))).unwrap(), (45.0, 9.0));
    assert!(paths.state_dir.join("location.json").is_file());
    assert_eq!(resolve(&cfg, &paths, &FakeLoc(None)).unwrap(), (45.0, 9.0));
}

#[test]
fn resolve_never_uses_null_island() {
    let home = TempDir::new().unwrap();
    let paths = Paths::under(home.path());
    let fixed = config(Mode::FollowSun, false, (0.0, 0.0));
    assert!(resolve(&fixed, &paths, &FakeLoc(None)).is_err());
    let auto = config(Mode::FollowSun, true, (0.0, 0.0));
    assert!(resolve(&auto, &paths, &FakeLoc(Some((0.0, 0.0)))).is_err());
}

#[test]
fn resolve_ignores_the_source_when_location_is_fixed() {
    let home = TempDir::new().unwrap();
    let paths = Paths::under(home.path());
    let cfg = config(Mode::FollowSun, false, (45.0, 9.0));
    assert_eq!(resolve(&cfg, &paths, &FakeLoc(Some((10.0, 10.0)))).unwrap(), (45.0, 9.0));
    assert!(!paths.state_dir.join("location.json").exists());
}

#[test]
fn ipinfo_response_parsing() {
    let body = r#"{"ip":"1.2.3.4","city":"Roma","loc":"41.9028,12.4964"}"#;
    assert_eq!(parse_ipinfo(body), Some((41.9028, 12.4964)));
    assert_eq!(parse_ipinfo(r#"{"error":"rate limited"}"#), None);
    assert_eq!(parse_ipinfo(r#"{"loc":"nonsense"}"#), None);
    assert_eq!(parse_ipinfo("<html>captive portal</html>"), None);
}
