pub fn human_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs >= 3600 {
        let hours = secs / 3600;
        let minutes = (secs % 3600) / 60;
        let seconds = secs % 60;
        let millis = d.subsec_millis();
        format!("{}h {:02}m {:02}.{:03}s", hours, minutes, seconds, millis)
    } else if secs >= 60 {
        let minutes = secs / 60;
        let seconds = secs % 60;
        let millis = d.subsec_millis();
        format!("{}m {:02}.{:03}s", minutes, seconds, millis)
    } else if secs >= 1 {
        format!("{:.3} s", d.as_secs_f64())
    } else if d.as_millis() >= 1 {
        format!("{} ms", d.as_millis())
    } else {
        format!("{} µs", d.as_micros())
    }
}
