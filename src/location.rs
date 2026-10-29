use std::fs;
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::config::Config;
use crate::paths::Paths;

const IPINFO_URL: &str = "https://ipinfo.io/json";
const CACHE_FILE: &str = "location.json";

pub trait LocationSource {
    fn current(&self) -> Option<(f64, f64)>;
}

/// Approximate position from the public IP address. Any failure is `None`.
pub struct IpInfoLocation;

impl LocationSource for IpInfoLocation {
    fn current(&self) -> Option<(f64, f64)> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(5)))
            .build()
            .into();
        let body = agent.get(IPINFO_URL).call().ok()?.body_mut().read_to_string().ok()?;
        parse_ipinfo(&body)
    }
}

/// Never answers; for callers that must not touch the network.
pub struct NoNetworkLocation;

impl LocationSource for NoNetworkLocation {
    fn current(&self) -> Option<(f64, f64)> {
        None
    }
}

pub fn parse_ipinfo(body: &str) -> Option<(f64, f64)> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let (lat, lon) = json.get("loc")?.as_str()?.split_once(',')?;
    Some((lat.trim().parse().ok()?, lon.trim().parse().ok()?))
}

/// (0, 0) means "not set", and anything out of range is not a place.
fn is_usable((lat, lon): (f64, f64)) -> bool {
    (lat, lon) != (0.0, 0.0) && (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)
}

/// The coordinates to use: the configured ones, or with `update_location` the current ones
/// (remembered in `state_dir`), falling back to the last remembered ones when offline.
pub fn resolve(cfg: &Config, paths: &Paths, src: &dyn LocationSource) -> Result<(f64, f64)> {
    if !cfg.update_location {
        if is_usable(cfg.coordinates) {
            return Ok(cfg.coordinates);
        }
        bail!("no usable coordinates in the config: {:?}", cfg.coordinates);
    }

    if let Some(current) = src.current().filter(|c| is_usable(*c)) {
        if let Err(e) = remember(paths, current) {
            log::warn!("cannot remember the location: {e:#}");
        }
        return Ok(current);
    }

    match recall(paths) {
        Some(last) => {
            log::warn!("cannot get the current location; using the last known one {last:?}");
            Ok(last)
        }
        None => bail!("cannot get the current location and none is remembered"),
    }
}

fn remember(paths: &Paths, location: (f64, f64)) -> Result<()> {
    fs::create_dir_all(&paths.state_dir)?;
    let file = paths.state_dir.join(CACHE_FILE);
    fs::write(&file, serde_json::to_vec(&location)?)
        .with_context(|| format!("writing {}", file.display()))
}

fn recall(paths: &Paths) -> Option<(f64, f64)> {
    let text = fs::read_to_string(paths.state_dir.join(CACHE_FILE)).ok()?;
    serde_json::from_str::<(f64, f64)>(&text)
        .ok()
        .filter(|c| is_usable(*c))
}
