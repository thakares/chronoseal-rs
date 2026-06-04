use shared::protocol::Fingerprint;

const MIN_ASPECT_RATIO: f64 = 0.5;
const MAX_ASPECT_RATIO: f64 = 3.0;
const MAX_DEVICE_PIXEL_RATIO: f64 = 5.0;
const MAX_HARDWARE_CONCURRENCY: u32 = 256;

/// Validates the browser fingerprint fields submitted by the client.
///
/// Checks basic screen aspect ratio thresholds, device pixel ratio limits,
/// and logical CPU core counts to reject anomaly fingerprints.
///
/// # Arguments
/// * `fp` - The client's hardware and screen layout fingerprint.
pub fn validate(fp: &Fingerprint) -> Result<(), Box<dyn std::error::Error>> {
    let ar: f64 = fp.aspect_ratio.parse().map_err(|_| "ar")?;
    if !ar.is_finite() || !(MIN_ASPECT_RATIO..=MAX_ASPECT_RATIO).contains(&ar) {
        return Err("aspect ratio".into());
    }

    let dpr: f64 = fp.device_pixel_ratio.parse().map_err(|_| "dpr")?;
    if !dpr.is_finite() || dpr <= 0.0 || dpr > MAX_DEVICE_PIXEL_RATIO {
        return Err("dpr".into());
    }

    if fp.hardware_concurrency == 0 || fp.hardware_concurrency > MAX_HARDWARE_CONCURRENCY {
        return Err("hw".into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint(
        aspect_ratio: impl Into<String>,
        device_pixel_ratio: impl Into<String>,
        hardware_concurrency: u32,
    ) -> Fingerprint {
        Fingerprint {
            aspect_ratio: aspect_ratio.into(),
            device_pixel_ratio: device_pixel_ratio.into(),
            hardware_concurrency,
        }
    }

    #[test]
    fn accepts_valid_fingerprint() {
        assert!(validate(&fingerprint("1.7777777778", "2", 8)).is_ok());
    }

    #[test]
    fn accepts_boundary_values() {
        assert!(validate(&fingerprint("0.5", "1", 1)).is_ok());
        assert!(validate(&fingerprint("3.0", "5.0", MAX_HARDWARE_CONCURRENCY)).is_ok());
    }

    #[test]
    fn rejects_invalid_aspect_ratios() {
        for aspect_ratio in ["not-a-number", "NaN", "inf", "0.49", "3.01"] {
            assert!(validate(&fingerprint(aspect_ratio, "2", 8)).is_err());
        }
    }

    #[test]
    fn rejects_invalid_device_pixel_ratios() {
        for device_pixel_ratio in ["not-a-number", "NaN", "inf", "0", "-1", "5.01"] {
            assert!(validate(&fingerprint("1.77", device_pixel_ratio, 8)).is_err());
        }
    }

    #[test]
    fn rejects_invalid_hardware_concurrency() {
        assert!(validate(&fingerprint("1.77", "2", 0)).is_err());
        assert!(validate(&fingerprint("1.77", "2", MAX_HARDWARE_CONCURRENCY + 1)).is_err());
    }
}
