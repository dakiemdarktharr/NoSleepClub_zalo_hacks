//! Explicit, offline runtime test using caller-approved DLL/model paths.
//! This checks integration/reset on silence; it is not a speech-quality eval.
use nora_audio::SileroVad;
use std::{error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let dll = PathBuf::from(args.next().ok_or("usage: silero_smoke DLL MODEL")?);
    let model = PathBuf::from(args.next().ok_or("usage: silero_smoke DLL MODEL")?);
    if args.next().is_some() {
        return Err("usage: silero_smoke DLL MODEL".into());
    }
    let environment = ort::init_from(dll)?.with_telemetry(false).build()?;
    let mut vad = SileroVad::load(&environment, model, 1)?;
    let first = vad.probability(&[0.0; 512])?;
    let second = vad.probability(&[0.0; 512])?;
    vad.reset();
    let reset = vad.probability(&[0.0; 512])?;
    if (reset - first).abs() > 1e-6 || [first, second, reset].iter().any(|p| *p >= 0.5) {
        return Err("Silero silence/reset smoke check failed".into());
    }
    if vad.probability(&[0.0; 511]).is_ok() || vad.probability(&[f32::NAN; 512]).is_ok() {
        return Err("Silero input-boundary check failed".into());
    }
    println!(
        "{{\"integration\":\"silero_cpu\",\"frames\":3,\"first_probability\":{first},\"second_probability\":{second},\"reset_probability\":{reset},\"reset_reproducible\":true}}"
    );
    Ok(())
}
