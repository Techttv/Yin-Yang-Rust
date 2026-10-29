use anyhow::Result;
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime, Utc};
use sunrise::{Coordinates, SolarDay, SolarEvent};

use crate::config::{Config, Mode};
use crate::location::{self, LocationSource};
use crate::paths::Paths;

/// True when `now` falls outside the light period that starts at `light` and ends at `dark`.
/// The light period may cross midnight (`light > dark`).
pub fn should_be_dark(now: NaiveTime, light: NaiveTime, dark: NaiveTime) -> bool {
    if light < dark {
        !(light <= now && now < dark)
    } else {
        dark <= now && now < light
    }
}

/// Sunrise and sunset of `date` at the given place, in UTC. Either one may fall on the previous
/// or next UTC date. `None` when the sun does not both rise and set that day, or on bad coordinates.
pub fn sun_times(lat: f64, lon: f64, date: NaiveDate) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let day = SolarDay::new(Coordinates::new(lat, lon)?, date);
    Some((
        day.event_time(SolarEvent::Sunrise)?,
        day.event_time(SolarEvent::Sunset)?,
    ))
}

/// The moments, as local times of day, at which the light and the dark theme start.
pub fn effective_times(
    cfg: &Config,
    loc: &dyn LocationSource,
    paths: &Paths,
    date: NaiveDate,
    offset: FixedOffset,
) -> Result<(NaiveTime, NaiveTime)> {
    if cfg.mode != Mode::FollowSun {
        return Ok(cfg.times);
    }

    let (lat, lon) = location::resolve(cfg, paths, loc)?;
    match sun_times(lat, lon, date) {
        Some((rise, set)) => Ok((
            rise.with_timezone(&offset).time(),
            set.with_timezone(&offset).time(),
        )),
        None => {
            log::warn!("no sunrise or sunset at {lat}, {lon} on {date}; using the manual times");
            Ok(cfg.times)
        }
    }
}
