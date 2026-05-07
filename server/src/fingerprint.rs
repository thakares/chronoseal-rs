use shared::protocol::Fingerprint;

pub fn validate(fp: &Fingerprint) -> Result<(), Box<dyn std::error::Error>> {
    let ar: f64 = fp.aspect_ratio.parse().map_err(|_| "ar")?;
    if ar < 0.5 || ar > 3.0 { return Err("aspect ratio".into()); }
    let dpr: f64 = fp.device_pixel_ratio.parse().map_err(|_| "dpr")?;
    if dpr <= 0.0 || dpr > 5.0 { return Err("dpr".into()); }
    if fp.hardware_concurrency == 0 { return Err("hw".into()); }
    Ok(())
}