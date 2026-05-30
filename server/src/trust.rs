use crate::config::Config;
use shared::protocol::EntropyData;

/// Validates the browser mouse cursor interaction path for bot/automation detection.
///
/// Evaluates mouse velocity and distance features, checks the total distance traversed,
/// checks for cursor pauses (low movement over high time diff), and enforces average cursor speeds.
///
/// # Arguments
/// * `data` - The client-supplied interaction entropy events.
/// * `config` - The server configuration boundaries.
pub fn validate_mouse(
    data: &EntropyData,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let events = &data.events;
    if !config.require_mouse_activity && events.is_empty() {
        return Ok(());
    }
    if events.len() < 3 {
        return Err("few events".into());
    }
    let mut total_dist = 0.0f64;
    let mut pauses = 0u32;
    for i in 1..events.len() {
        let p = &events[i - 1];
        let c = &events[i];
        let dx = c.x - p.x;
        let dy = c.y - p.y;
        let dt = (c.timestamp_ms - p.timestamp_ms).max(1.0);
        let dist = (dx * dx + dy * dy).sqrt();
        total_dist += dist;
        if dist < 0.2 && dt > 50.0 {
            pauses += 1;
        }
    }
    if total_dist < config.min_mouse_total_dist {
        return Err("insufficient distance".into());
    }
    // Speed in px/ms: total distance over elapsed wall-clock time of the event window.
    let total_time_ms = (events.last().unwrap().timestamp_ms - events[0].timestamp_ms).max(1.0);
    let avg_speed = total_dist / total_time_ms;
    if avg_speed > config.max_mouse_avg_speed {
        return Err("speed too high".into());
    }
    if pauses < config.min_pause_count {
        return Err("no pause".into());
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use shared::protocol::MouseEvent;

    fn get_default_config() -> Config {
        Config {
            min_mouse_total_dist: 10.0,
            max_mouse_avg_speed: 2.0,
            min_pause_count: 1,
            require_mouse_activity: true,
            ..Config::default()
        }
    }

    #[test]
    fn test_validate_mouse_success() {
        let config = get_default_config();
        // Mouse moves from (0,0) to (5,0) then (15,0) with a pause
        let events = vec![
            MouseEvent {
                x: 0.0,
                y: 0.0,
                timestamp_ms: 100.0,
            },
            MouseEvent {
                x: 5.0,
                y: 0.0,
                timestamp_ms: 200.0,
            },
            // Pause here (dist = 0, time diff = 100ms > 50ms)
            MouseEvent {
                x: 5.0,
                y: 0.0,
                timestamp_ms: 300.0,
            },
            MouseEvent {
                x: 15.0,
                y: 0.0,
                timestamp_ms: 400.0,
            },
        ];
        let data = EntropyData { events };
        assert!(validate_mouse(&data, &config).is_ok());
    }

    #[test]
    fn test_validate_mouse_insufficient_events() {
        let config = get_default_config();
        let events = vec![
            MouseEvent {
                x: 0.0,
                y: 0.0,
                timestamp_ms: 100.0,
            },
            MouseEvent {
                x: 5.0,
                y: 0.0,
                timestamp_ms: 200.0,
            },
        ];
        let data = EntropyData { events };
        let res = validate_mouse(&data, &config);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "few events");
    }

    #[test]
    fn test_validate_mouse_insufficient_distance() {
        let config = get_default_config();
        // Total distance is only 5.0 < 10.0
        let events = vec![
            MouseEvent {
                x: 0.0,
                y: 0.0,
                timestamp_ms: 100.0,
            },
            MouseEvent {
                x: 2.0,
                y: 0.0,
                timestamp_ms: 200.0,
            },
            MouseEvent {
                x: 2.0,
                y: 0.0,
                timestamp_ms: 300.0,
            },
            MouseEvent {
                x: 5.0,
                y: 0.0,
                timestamp_ms: 400.0,
            },
        ];
        let data = EntropyData { events };
        let res = validate_mouse(&data, &config);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "insufficient distance");
    }

    #[test]
    fn test_validate_mouse_too_fast() {
        let config = get_default_config();
        // Distance is 200.0, time difference is 70ms -> speed 2.85 > 2.0
        let events = vec![
            MouseEvent {
                x: 0.0,
                y: 0.0,
                timestamp_ms: 100.0,
            },
            MouseEvent {
                x: 100.0,
                y: 0.0,
                timestamp_ms: 105.0,
            },
            MouseEvent {
                x: 100.0,
                y: 0.0,
                timestamp_ms: 165.0,
            }, // pause
            MouseEvent {
                x: 200.0,
                y: 0.0,
                timestamp_ms: 170.0,
            },
        ];
        let data = EntropyData { events };
        let res = validate_mouse(&data, &config);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "speed too high");
    }

    #[test]
    fn test_validate_mouse_no_pauses() {
        let config = get_default_config();
        // Constant movement without any pause
        let events = vec![
            MouseEvent {
                x: 0.0,
                y: 0.0,
                timestamp_ms: 100.0,
            },
            MouseEvent {
                x: 5.0,
                y: 0.0,
                timestamp_ms: 200.0,
            },
            MouseEvent {
                x: 10.0,
                y: 0.0,
                timestamp_ms: 300.0,
            },
            MouseEvent {
                x: 15.0,
                y: 0.0,
                timestamp_ms: 400.0,
            },
        ];
        let data = EntropyData { events };
        let res = validate_mouse(&data, &config);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "no pause");
    }

    #[test]
    fn test_validate_mouse_require_activity_toggle() {
        let mut config = get_default_config();
        config.require_mouse_activity = false;

        let data = EntropyData { events: vec![] };
        assert!(validate_mouse(&data, &config).is_ok());

        config.require_mouse_activity = true;
        assert!(validate_mouse(&data, &config).is_err());
    }
}
