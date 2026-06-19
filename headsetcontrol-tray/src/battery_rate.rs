use std::time::{Duration, Instant};

/// Tracks battery level changes over time to estimate remaining time.
///
/// IMPORTANT charging-curve caveat: this headset (and many Li-ion devices)
/// reports 99% for an extended constant-voltage trickle phase before finally
/// ticking to 100%, so the LAST percent step while charging will measure as
/// disproportionately long. We label it as a rough estimate in the UI to
/// account for this.
pub struct BatteryRateTracker {
    last_level: Option<i32>,
    last_level_changed_at: Option<Instant>,
    last_charging_state: Option<bool>,
    discharge_seconds_per_percent: Option<f32>, // rolling estimate
    charge_seconds_per_percent: Option<f32>,    // rolling estimate
}

impl BatteryRateTracker {
    pub fn new() -> Self {
        Self {
            last_level: None,
            last_level_changed_at: None,
            last_charging_state: None,
            discharge_seconds_per_percent: None,
            charge_seconds_per_percent: None,
        }
    }

    pub fn update(&mut self, new_level: i32, charging: bool) {
        let now = Instant::now();

        // If charging state flipped, reset tracking to avoid mixing charge/discharge timing.
        if self.last_charging_state != Some(charging) {
            self.last_charging_state = Some(charging);
            self.last_level = Some(new_level);
            self.last_level_changed_at = Some(now);
            return;
        }

        if let (Some(last_level), Some(last_changed_at)) = (self.last_level, self.last_level_changed_at) {
            if new_level != last_level {
                let duration = now.duration_since(last_changed_at);
                let percent_delta = (new_level - last_level).abs();

                if percent_delta > 0 {
                    let seconds_per_percent = duration.as_secs_f32() / percent_delta as f32;

                    let estimate_ref = if charging {
                        &mut self.charge_seconds_per_percent
                    } else {
                        &mut self.discharge_seconds_per_percent
                    };

                    // Use Exponential Moving Average (EMA) to smooth out readings.
                    if let Some(old_estimate) = *estimate_ref {
                        *estimate_ref = Some(0.3 * seconds_per_percent + 0.7 * old_estimate);
                    } else {
                        *estimate_ref = Some(seconds_per_percent);
                    }
                }

                self.last_level = Some(new_level);
                self.last_level_changed_at = Some(now);
            }
        } else {
            self.last_level = Some(new_level);
            self.last_level_changed_at = Some(now);
        }
    }

    pub fn estimated_remaining(&self, current_level: i32, charging: bool) -> Option<Duration> {
        let seconds_per_percent = if charging {
            self.charge_seconds_per_percent?
        } else {
            self.discharge_seconds_per_percent?
        };

        let remaining_percent = if charging {
            100 - current_level
        } else {
            current_level
        };

        if remaining_percent <= 0 {
            return Some(Duration::ZERO);
        }

        Some(Duration::from_secs_f32(seconds_per_percent * remaining_percent as f32))
    }
}
