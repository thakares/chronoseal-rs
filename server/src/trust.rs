use shared::protocol::EntropyData;

pub fn validate_mouse(data: &EntropyData) -> Result<(), Box<dyn std::error::Error>> {
    let events = &data.events;
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
    if total_dist < shared::constants::MIN_MOUSE_TOTAL_DIST {
        return Err("insufficient distance".into());
    }
    // Speed in px/ms: total distance over elapsed wall-clock time of the event window.
    let total_time_ms =
        (events.last().unwrap().timestamp_ms - events[0].timestamp_ms).max(1.0);
    let avg_speed = total_dist / total_time_ms;
    if avg_speed > shared::constants::MAX_MOUSE_AVG_SPEED {
        return Err("speed too high".into());
    }
    if pauses < shared::constants::MIN_PAUSE_COUNT {
        return Err("no pause".into());
    }
    Ok(())
}