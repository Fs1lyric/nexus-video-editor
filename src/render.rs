//! ffmpeg orchestration: normalise every clip to identical codec parameters,
//! then concatenate them with the stream-copy concat demuxer.
//!
//! Normalising first is slower (two encodes) but it is the reliable way to join
//! arbitrary sources — different resolutions, frame rates, or a missing audio
//! track no longer break the join.

use anyhow::{bail, Context, Result};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::media::{ffmpeg_bin, probe};
use crate::project::Project;

/// Progress sink: `callback(fraction_0_to_1, human_message)`.
pub type ProgressFn<'a> = &'a mut dyn FnMut(f32, &str);

/// Render a whole project to `out` (an `.mp4` path).
pub fn export_project(project: &Project, out: &Path, prog: ProgressFn) -> Result<()> {
    let clips = &project.timeline.clips;
    if clips.is_empty() {
        bail!("timeline is empty — add some clips first");
    }

    let total = project.duration().max(0.001);
    let tmp = TmpDir::new()?;
    let mut parts = Vec::with_capacity(clips.len());
    let mut done = 0.0_f64;

    for (i, c) in clips.iter().enumerate() {
        if !c.source.exists() {
            bail!("missing source file: {}", c.source.display());
        }
        let d = c.duration();
        if d <= 0.0 {
            bail!(
                "clip {} has zero length (in {} >= out {})",
                i + 1,
                c.src_in,
                c.src_out
            );
        }

        let part = tmp.path().join(format!("part{i:04}.mp4"));
        let base = (done / total) as f32 * 0.9;
        let span = (d / total) as f32 * 0.9;
        prog(base, &format!("normalizing clip {}/{}", i + 1, clips.len()));
        normalize_clip(
            &c.source,
            c.src_in,
            d,
            project.width,
            project.height,
            project.fps,
            &part,
            base,
            span,
            prog,
        )?;
        parts.push(part);
        done += d;
    }

    prog(0.9, "joining clips");
    concat_copy(&parts, out)?;
    prog(1.0, "done");
    Ok(())
}

/// Trim a single file to `[start, end)` (end `<= 0` means "to the end") and
/// re-encode. Frame-accurate at the cut points.
pub fn trim(src: &Path, start: f64, end: f64, out: &Path, prog: ProgressFn) -> Result<()> {
    let info = probe(src)?;
    let end = if end <= 0.0 { info.duration } else { end };
    if end <= start {
        bail!("end ({end}) must be greater than start ({start})");
    }
    let tmp = TmpDir::new()?;
    let part = tmp.path().join("part.mp4");
    normalize_clip(
        src,
        start,
        end - start,
        info.width.max(2),
        info.height.max(2),
        if info.fps > 0.0 { info.fps } else { 30.0 },
        &part,
        0.0,
        1.0,
        prog,
    )?;
    std::fs::copy(&part, out).with_context(|| format!("writing {}", out.display()))?;
    Ok(())
}

/// Concatenate two or more standalone files, scaling each to `w x h @ fps`.
pub fn concat(
    files: &[PathBuf],
    out: &Path,
    w: u32,
    h: u32,
    fps: f64,
    prog: ProgressFn,
) -> Result<()> {
    if files.len() < 2 {
        bail!("need at least two input files");
    }
    let tmp = TmpDir::new()?;
    let mut parts = Vec::with_capacity(files.len());
    let n = files.len() as f32;

    for (i, f) in files.iter().enumerate() {
        if !f.exists() {
            bail!("missing file: {}", f.display());
        }
        let info = probe(f)?;
        let part = tmp.path().join(format!("p{i:04}.mp4"));
        let base = i as f32 / n * 0.9;
        prog(base, &format!("normalizing {}/{}", i + 1, files.len()));
        normalize_clip(f, 0.0, info.duration, w, h, fps, &part, base, 0.9 / n, prog)?;
        parts.push(part);
    }

    prog(0.9, "joining clips");
    concat_copy(&parts, out)?;
    prog(1.0, "done");
    Ok(())
}

// ---------------------------------------------------------------------------

fn video_filter(w: u32, h: u32, fps: f64) -> String {
    // Fit inside the frame, pad the rest black, fix SAR, resample fps, 8-bit 4:2:0.
    format!(
        "scale={w}:{h}:force_original_aspect_ratio=decrease,\
         pad={w}:{h}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps={fps},format=yuv420p"
    )
}

#[allow(clippy::too_many_arguments)]
fn normalize_clip(
    src: &Path,
    src_in: f64,
    dur: f64,
    w: u32,
    h: u32,
    fps: f64,
    out: &Path,
    base: f32,
    span: f32,
    prog: ProgressFn,
) -> Result<()> {
    let info = probe(src)?;

    let mut args: Vec<String> = vec![
        "-y".into(),
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-progress".into(),
        "pipe:1".into(),
        "-nostats".into(),
        "-ss".into(),
        fmt_time(src_in),
        "-i".into(),
        src.to_string_lossy().into_owned(),
    ];

    // Give clips with no audio a silent stereo track so the concat stays aligned.
    if !info.has_audio {
        args.extend([
            "-f".into(),
            "lavfi".into(),
            "-i".into(),
            "anullsrc=channel_layout=stereo:sample_rate=48000".into(),
        ]);
    }

    args.extend(["-t".into(), fmt_time(dur)]);
    args.extend(["-vf".into(), video_filter(w, h, fps)]);

    if info.has_audio {
        args.extend(["-map".into(), "0:v:0".into(), "-map".into(), "0:a:0".into()]);
    } else {
        args.extend(["-map".into(), "0:v:0".into(), "-map".into(), "1:a:0".into()]);
    }

    args.extend([
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "veryfast".into(),
        "-crf".into(),
        "20".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "192k".into(),
        "-ar".into(),
        "48000".into(),
        "-ac".into(),
        "2".into(),
        "-shortest".into(),
        "-movflags".into(),
        "+faststart".into(),
        out.to_string_lossy().into_owned(),
    ]);

    run_ffmpeg(&args, dur, base, span, prog)
}

fn concat_copy(parts: &[PathBuf], out: &Path) -> Result<()> {
    if let [only] = parts {
        std::fs::copy(only, out).with_context(|| format!("writing {}", out.display()))?;
        return Ok(());
    }

    let list = out.with_file_name(format!(".nexus-concat-{}.txt", nonce()));
    let body: String = parts
        .iter()
        .map(|p| format!("file '{}'\n", p.to_string_lossy().replace('\'', "'\\''")))
        .collect();
    std::fs::write(&list, body)?;

    let args: Vec<String> = vec![
        "-y".into(),
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-f".into(),
        "concat".into(),
        "-safe".into(),
        "0".into(),
        "-i".into(),
        list.to_string_lossy().into_owned(),
        "-c".into(),
        "copy".into(),
        "-movflags".into(),
        "+faststart".into(),
        out.to_string_lossy().into_owned(),
    ];

    let mut noop = |_p: f32, _m: &str| {};
    let result = run_ffmpeg(&args, 0.0, 0.9, 0.1, &mut noop);
    let _ = std::fs::remove_file(&list);
    result
}

fn run_ffmpeg(
    args: &[String],
    stage_total: f64,
    base: f32,
    span: f32,
    prog: ProgressFn,
) -> Result<()> {
    let mut child = Command::new(ffmpeg_bin())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("could not spawn ffmpeg — is it installed and on PATH?")?;

    // ffmpeg writes `key=value` progress lines to stdout because of `-progress pipe:1`.
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(us) = line.strip_prefix("out_time_us=") {
                if stage_total > 0.0 {
                    if let Ok(us) = us.trim().parse::<f64>() {
                        let frac = (us / 1_000_000.0 / stage_total).clamp(0.0, 1.0) as f32;
                        prog(base + span * frac, "encoding");
                    }
                }
            }
        }
    }

    let output = child.wait_with_output()?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let tail: Vec<&str> = err.lines().rev().take(15).collect::<Vec<_>>();
        let msg: Vec<&str> = tail.into_iter().rev().collect();
        bail!("ffmpeg failed:\n{}", msg.join("\n"));
    }
    Ok(())
}

fn fmt_time(seconds: f64) -> String {
    format!("{:.3}", seconds.max(0.0))
}

fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// A temp directory that deletes itself on drop.
struct TmpDir(PathBuf);

impl TmpDir {
    fn new() -> Result<Self> {
        let dir = std::env::temp_dir().join(format!("nexus-{}-{}", std::process::id(), nonce()));
        std::fs::create_dir_all(&dir)?;
        Ok(Self(dir))
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Parse `"12.5"` or `"1:02:03.5"` or `"02:03"` into seconds.
pub fn parse_time(s: &str) -> Result<f64> {
    let s = s.trim();
    if let Ok(v) = s.parse::<f64>() {
        return Ok(v);
    }
    let mut secs = 0.0;
    for part in s.split(':') {
        let n: f64 = part
            .parse()
            .with_context(|| format!("'{s}' is not a valid time (use seconds or HH:MM:SS)"))?;
        secs = secs * 60.0 + n;
    }
    Ok(secs)
}

#[cfg(test)]
mod tests {
    use super::parse_time;

    #[test]
    fn parses_plain_seconds_and_clock_times() {
        assert_eq!(parse_time("12.5").unwrap(), 12.5);
        assert_eq!(parse_time(" 90 ").unwrap(), 90.0);
        assert_eq!(parse_time("02:03").unwrap(), 123.0);
        assert_eq!(parse_time("1:02:03.5").unwrap(), 3723.5);
        assert!(parse_time("nope").is_err());
    }
}
