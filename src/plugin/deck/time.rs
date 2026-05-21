use mlua::prelude::*;

use chrono::Datelike;

fn relative_phrase(delta_seconds: i64) -> String {
    let future = delta_seconds > 0;
    let abs = delta_seconds.unsigned_abs();

    let phrase = if abs < 60 {
        "1 minute".to_string()
    } else if abs < 60 * 60 {
        format!("{} minutes", abs / 60)
    } else if abs < 24 * 60 * 60 {
        let hours = abs / 3600;
        if hours <= 1 {
            "1 hour".to_string()
        } else {
            format!("{} hours", hours)
        }
    } else if abs < 36 * 60 * 60 {
        "yesterday".to_string()
    } else if abs < 7 * 24 * 60 * 60 {
        format!("{} days", abs / (24 * 3600))
    } else if abs < 14 * 24 * 60 * 60 {
        "last week".to_string()
    } else if abs < 30 * 24 * 60 * 60 {
        format!("{} weeks", abs / (7 * 24 * 3600))
    } else if abs < 45 * 24 * 60 * 60 {
        "last month".to_string()
    } else if abs < 365 * 24 * 60 * 60 {
        format!("{} months", abs / (30 * 24 * 3600))
    } else if abs < 545 * 24 * 60 * 60 {
        "last year".to_string()
    } else {
        format!("{} years", abs / (365 * 24 * 3600))
    };

    match phrase.as_str() {
        "yesterday" => {
            if future {
                "tomorrow".to_string()
            } else {
                phrase
            }
        }
        "last week" => {
            if future {
                "next week".to_string()
            } else {
                phrase
            }
        }
        "last month" => {
            if future {
                "next month".to_string()
            } else {
                phrase
            }
        }
        "last year" => {
            if future {
                "next year".to_string()
            } else {
                phrase
            }
        }
        _ => {
            if future {
                format!("in {}", phrase)
            } else {
                format!("{} ago", phrase)
            }
        }
    }
}

/// Create the deck.time table with time-related functions
pub(super) fn new_table(lua: &Lua) -> mlua::Result<LuaTable> {
    // Parse an ISO 8601 datetime string and return Unix timestamp
    let parse = lua.create_function(|_, time_str: String| {
        use chrono::{DateTime, NaiveDate, NaiveDateTime};

        // Try RFC 3339 / ISO 8601 first (handles most common formats)
        if let Ok(dt) = DateTime::parse_from_rfc3339(&time_str) {
            return Ok(dt.timestamp());
        }

        // Remove fractional seconds for formats with .123
        let _time_str_no_ms = time_str.replace(".000", "").replace(r"\.\d+", "");

        // Try parsing with timezone info
        let tz_formats = [
            "%Y-%m-%dT%H:%M:%SZ",       // 2023-12-25T15:30:45Z
            "%Y-%m-%dT%H:%M:%S%z",     // 2023-12-25T15:30:45+08:00
            "%Y-%m-%d %H:%M:%S%z",     // 2023-12-25 15:30:45+08:00
            "%Y-%m-%d %H:%M%z",        // 2026-02-16 18:09+08:00 (himalaya format without seconds)
        ];

        for fmt in &tz_formats {
            if let Ok(dt) = DateTime::parse_from_str(&time_str, fmt) {
                return Ok(dt.timestamp());
            }
        }

        // Try naive datetime formats (assume UTC)
        let naive_formats = [
            "%Y-%m-%dT%H:%M:%S",       // 2023-12-25T15:30:45
            "%Y-%m-%d %H:%M:%S",       // 2023-12-25 15:30:45
            "%Y-%m-%d",                // 2023-12-25
        ];

        for fmt in &naive_formats {
            if let Ok(dt) = NaiveDateTime::parse_from_str(&time_str, fmt) {
                let dt_utc: DateTime<chrono::Utc> = DateTime::from_naive_utc_and_offset(dt, chrono::Utc);
                return Ok(dt_utc.timestamp());
            }
            // Try as date only
            if let Ok(date) = NaiveDate::parse_from_str(&time_str, fmt) {
                let dt = date.and_hms_opt(0, 0, 0).unwrap();
                let dt_utc: DateTime<chrono::Utc> = DateTime::from_naive_utc_and_offset(dt, chrono::Utc);
                return Ok(dt_utc.timestamp());
            }
        }

        // Try RFC 2822 format
        if let Ok(dt) = DateTime::parse_from_rfc2822(&time_str) {
            return Ok(dt.timestamp());
        }

        Err(LuaError::RuntimeError(format!(
            "Failed to parse time string: '{}'. Supported formats: ISO 8601 (e.g., 2023-12-25T15:30:45Z, 2023-12-25T15:30:45+08:00), RFC 3339, RFC 2822",
            time_str
        )))
    })?.into_lua(lua)?;

    // Get current Unix timestamp
    let now = lua
        .create_function(|_, ()| Ok(chrono::Utc::now().timestamp()))?
        .into_lua(lua)?;

    // Format Unix timestamp in the local timezone to ISO 8601 string (or custom format)
    let format = lua
        .create_function(|_, (timestamp, format_opt): (i64, Option<String>)| {
            use chrono::{DateTime, Local};

            let dt_utc = DateTime::<chrono::Utc>::from_timestamp(timestamp, 0)
                .ok_or_else(|| LuaError::RuntimeError("Invalid timestamp".to_string()))?;
            let dt_local = dt_utc.with_timezone(&Local);

            match format_opt.as_deref() {
                // Compact format: adaptively choose format based on local time distance
                Some("compact") => {
                    let now_local = Local::now();

                    // Check if it's today in local timezone
                    let is_today = dt_local.date_naive() == now_local.date_naive();

                    if is_today {
                        // Today: show time only (HH:MM)
                        Ok(dt_local.format("%H:%M").to_string())
                    } else if dt_local.year() != now_local.year() {
                        // Different year: show YYYY/MM
                        Ok(dt_local.format("%Y/%m").to_string())
                    } else {
                        // Same year but not today: show MM/DD
                        Ok(dt_local.format("%m/%d").to_string())
                    }
                }
                Some("relative") => {
                    let now = chrono::Utc::now().timestamp();
                    Ok(relative_phrase(dt_utc.timestamp() - now))
                }
                Some(fmt) => Ok(dt_local.format(fmt).to_string()),
                None => Ok(dt_local.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
            }
        })?
        .into_lua(lua)?;

    lua.create_table_from([("parse", parse), ("now", now), ("format", format)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;

    #[test]
    fn test_time_parse_iso8601() {
        let lua = Lua::new();
        let time_table = new_table(&lua).unwrap();

        // Test parsing ISO 8601 with Z timezone
        let parse_fn: mlua::Function = time_table.get("parse").unwrap();
        let ts: i64 = parse_fn.call("2023-12-25T15:30:45Z").unwrap();

        // Verify it's a valid timestamp (should be around Dec 2023)
        assert!(ts > 1700000000); // After Dec 2023
        assert!(ts < 1800000000); // Before 2027

        // Test parsing ISO 8601 with offset timezone
        let ts: i64 = parse_fn.call("2023-12-25T15:30:45+08:00").unwrap();
        // +08:00 should be 8 hours earlier in UTC
        assert!(ts > 1700000000);

        // Test parsing with milliseconds
        let ts_ms: i64 = parse_fn.call("2023-12-25T15:30:45.123Z").unwrap();
        // Should be a valid timestamp (fractional seconds ignored)
        assert!(ts_ms > 1700000000);

        // Test parsing date only
        let ts_date: i64 = parse_fn.call("2023-12-25").unwrap();
        // Should be midnight UTC on that date
        assert!(ts_date > 1700000000);
    }

    #[test]
    fn test_time_now() {
        let lua = Lua::new();
        let time_table = new_table(&lua).unwrap();

        let now_fn: mlua::Function = time_table.get("now").unwrap();
        let ts: i64 = now_fn.call(()).unwrap();

        // Should be close to current time (within 10 seconds)
        let expected = chrono::Utc::now().timestamp();
        assert!((ts - expected).abs() < 10);
    }

    #[test]
    fn test_time_format() {
        let lua = Lua::new();
        let time_table = new_table(&lua).unwrap();

        // Use a known timestamp: 2023-01-01 00:00:00 UTC = 1672531200
        let known_ts = 1672531200;
        let expected_local = chrono::DateTime::<chrono::Utc>::from_timestamp(known_ts, 0)
            .unwrap()
            .with_timezone(&chrono::Local);

        let format_fn: mlua::Function = time_table.get("format").unwrap();
        let formatted: String = format_fn.call((known_ts, None::<String>)).unwrap();
        assert_eq!(
            formatted,
            expected_local.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        );

        // Custom format should also use local timezone
        let formatted: String = format_fn
            .call((known_ts, Some("%Y-%m-%d".to_string())))
            .unwrap();
        assert_eq!(formatted, expected_local.format("%Y-%m-%d").to_string());
    }

    #[test]
    fn test_time_parse_himalaya_formats() {
        let lua = Lua::new();
        let time_table = new_table(&lua).unwrap();
        let parse_fn: mlua::Function = time_table.get("parse").unwrap();

        // Test himalaya date formats
        let formats = ["2026-02-16 18:09+08:00", "2026-02-16T18:09:00+08:00"];

        for fmt in &formats {
            let ts: mlua::Result<i64> = parse_fn.call(*fmt);
            assert!(ts.is_ok(), "Failed to parse format: {}", fmt);
            let timestamp = ts.unwrap();
            // Verify it's a reasonable timestamp (year 2026)
            assert!(
                timestamp > 1700000000,
                "Timestamp too early for format: {}",
                fmt
            );
            assert!(
                timestamp < 2000000000,
                "Timestamp too late for format: {}",
                fmt
            );
        }
    }

    #[test]
    fn test_time_parse_invalid() {
        let lua = Lua::new();
        let time_table = new_table(&lua).unwrap();

        let parse_fn: mlua::Function = time_table.get("parse").unwrap();
        let result: mlua::Result<i64> = parse_fn.call("invalid-date");
        assert!(result.is_err());
    }

    #[test]
    fn test_time_format_compact_today() {
        use chrono::Local;

        let lua = Lua::new();
        let time_table = new_table(&lua).unwrap();

        let format_fn: mlua::Function = time_table.get("format").unwrap();

        // Use current time (should be today)
        let now = Local::now();
        let now_timestamp = now.timestamp();

        let formatted: String = format_fn
            .call((now_timestamp, Some("compact".to_string())))
            .unwrap();
        // Should be in HH:MM format (no date part)
        assert!(formatted.matches(char::is_numeric).count() <= 5);
        assert!(formatted.len() == 5); // "HH:MM"
        assert!(formatted.contains(':'));
    }

    #[test]
    fn test_time_format_compact_this_year() {
        use chrono::{Duration, Local};

        let lua = Lua::new();
        let time_table = new_table(&lua).unwrap();

        let format_fn: mlua::Function = time_table.get("format").unwrap();

        // Use a date from this year but not today
        let now = Local::now();
        let earlier_this_year = now - Duration::days(30); // 30 days ago
        let timestamp = earlier_this_year.timestamp();

        let formatted: String = format_fn
            .call((timestamp, Some("compact".to_string())))
            .unwrap();
        // Should be in MM/DD format
        assert!(formatted.len() == 5); // "MM/DD"
        assert!(formatted.contains('/'));
        assert!(!formatted.contains(':'));
    }

    #[test]
    fn test_time_format_compact_previous_year() {
        use chrono::{Duration, Local};

        let lua = Lua::new();
        let time_table = new_table(&lua).unwrap();

        let format_fn: mlua::Function = time_table.get("format").unwrap();

        // Use a date from last year
        let now = Local::now();
        let last_year = now - Duration::days(400); // Over a year ago
        let timestamp = last_year.timestamp();

        let formatted: String = format_fn
            .call((timestamp, Some("compact".to_string())))
            .unwrap();
        // Should be in YYYY/MM format
        assert!(formatted.len() == 7); // "YYYY/MM"
        assert!(formatted.contains('/'));
        assert!(!formatted.contains(':'));
    }

    #[test]
    fn test_time_format_relative_past() {
        let now = chrono::Utc::now().timestamp();

        assert_eq!(relative_phrase(now - now), "1 minute ago");
        assert_eq!(relative_phrase((now - 47 * 60) - now), "47 minutes ago");
        assert_eq!(relative_phrase((now - 60 * 60) - now), "1 hour ago");
        assert_eq!(relative_phrase((now - 2 * 60 * 60) - now), "2 hours ago");
        assert_eq!(relative_phrase((now - 24 * 60 * 60) - now), "yesterday");
        assert_eq!(
            relative_phrase((now - 2 * 24 * 60 * 60) - now),
            "2 days ago"
        );
        assert_eq!(relative_phrase((now - 8 * 24 * 60 * 60) - now), "last week");
        assert_eq!(
            relative_phrase((now - 14 * 24 * 60 * 60) - now),
            "2 weeks ago"
        );
        assert_eq!(
            relative_phrase((now - 35 * 24 * 60 * 60) - now),
            "last month"
        );
    }

    #[test]
    fn test_time_format_relative_future() {
        let now = chrono::Utc::now().timestamp();

        assert_eq!(relative_phrase((now + 47 * 60) - now), "in 47 minutes");
        assert_eq!(relative_phrase((now + 60 * 60) - now), "in 1 hour");
        assert_eq!(relative_phrase((now + 2 * 60 * 60) - now), "in 2 hours");
        assert_eq!(relative_phrase((now + 24 * 60 * 60) - now), "tomorrow");
        assert_eq!(relative_phrase((now + 2 * 24 * 60 * 60) - now), "in 2 days");
        assert_eq!(relative_phrase((now + 8 * 24 * 60 * 60) - now), "next week");
        assert_eq!(
            relative_phrase((now + 14 * 24 * 60 * 60) - now),
            "in 2 weeks"
        );
        assert_eq!(
            relative_phrase((now + 35 * 24 * 60 * 60) - now),
            "next month"
        );
    }

    #[test]
    fn test_time_format_relative_via_lua_api() {
        use chrono::Duration;

        let lua = Lua::new();
        let time_table = new_table(&lua).unwrap();
        let format_fn: mlua::Function = time_table.get("format").unwrap();
        let timestamp = (chrono::Utc::now() - Duration::minutes(47)).timestamp();

        let formatted: String = format_fn
            .call((timestamp, Some("relative".to_string())))
            .unwrap();
        assert_eq!(formatted, "47 minutes ago");
    }
}
