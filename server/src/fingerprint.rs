use shared::protocol::Fingerprint;

/// Validates the browser fingerprint fields submitted by the client.
///
/// Checks basic screen aspect ratio thresholds, device pixel ratio limits,
/// and logical CPU core counts to reject anomaly fingerprints.
///
/// # Arguments
/// * `fp` - The client's hardware and screen layout fingerprint.
pub fn validate(fp: &Fingerprint) -> Result<(), Box<dyn std::error::Error>> {
    let ar: f64 = fp.aspect_ratio.parse().map_err(|_| "ar")?;
    if !(0.5..=3.0).contains(&ar) {
        return Err("aspect ratio".into());
    }
    let dpr: f64 = fp.device_pixel_ratio.parse().map_err(|_| "dpr")?;
    if dpr <= 0.0 || dpr > 5.0 {
        return Err("dpr".into());
    }
    if fp.hardware_concurrency == 0 {
        return Err("hw".into());
    }
    Ok(())
}

