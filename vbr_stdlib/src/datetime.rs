//! Wraps `chrono`. A `DateTime` value is a moment in local time — construct one
//! with `DateTime::now()` or `DateTime::parse(...)`, then call methods on it.

use chrono::{DateTime as ChronoDateTime, Datelike, Duration, Local, NaiveDateTime};

#[derive(Clone, Copy)]
pub struct DateTime(ChronoDateTime<Local>);

impl DateTime {
    /// The current local date and time.
    /// VBA equivalent: Now()
    pub fn now() -> DateTime {
        DateTime(Local::now())
    }

    /// Parse a datetime from a string and pattern, in local time.
    /// VBA equivalent: CDate()
    pub fn parse(text: &str, pattern: &str) -> Result<DateTime, String> {
        let naive = NaiveDateTime::parse_from_str(text, pattern).map_err(|e| e.to_string())?;
        naive
            .and_local_timezone(Local)
            .single()
            .ok_or_else(|| "ambiguous or invalid local time".to_string())
            .map(DateTime)
    }

    /// Format this datetime as a string.
    /// VBA equivalent: Format(date, "pattern")
    pub fn format(&self, pattern: &str) -> String {
        self.0.format(pattern).to_string()
    }

    /// A new datetime `days` later.
    /// VBA equivalent: DateAdd("d", n, date)
    pub fn add_days(&self, days: i64) -> DateTime {
        DateTime(self.0 + Duration::days(days))
    }

    /// A new datetime `hours` later.
    /// VBA equivalent: DateAdd("h", n, date)
    pub fn add_hours(&self, hours: i64) -> DateTime {
        DateTime(self.0 + Duration::hours(hours))
    }

    /// A new datetime `minutes` later.
    /// VBA equivalent: DateAdd("n", n, date)
    pub fn add_minutes(&self, minutes: i64) -> DateTime {
        DateTime(self.0 + Duration::minutes(minutes))
    }

    /// Whole days from this datetime to `other`.
    /// VBA equivalent: DateDiff("d", date1, date2)
    pub fn diff_days(&self, other: &DateTime) -> i64 {
        (other.0 - self.0).num_days()
    }

    /// Whole hours from this datetime to `other`.
    /// VBA equivalent: DateDiff("h", date1, date2)
    pub fn diff_hours(&self, other: &DateTime) -> i64 {
        (other.0 - self.0).num_hours()
    }

    /// The year.
    /// VBA equivalent: Year(date)
    pub fn year(&self) -> i32 {
        self.0.year()
    }

    /// The month (1–12).
    /// VBA equivalent: Month(date)
    pub fn month(&self) -> u32 {
        self.0.month()
    }

    /// The day of the month (1–31).
    /// VBA equivalent: Day(date)
    pub fn day(&self) -> u32 {
        self.0.day()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_now_and_format() {
        assert_eq!(DateTime::now().format("%Y-%m-%d").len(), 10);
    }

    #[test]
    fn test_add_and_diff() {
        let now = DateTime::now();
        let tomorrow = now.add_days(1);
        assert_eq!(now.diff_days(&tomorrow), 1);
    }

    #[test]
    fn test_parse_and_parts() {
        let dt = DateTime::parse("2024-01-15 10:30:00", "%Y-%m-%d %H:%M:%S").unwrap();
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 15);
    }
}
