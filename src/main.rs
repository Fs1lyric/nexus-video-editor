//! `nexus` — run with a subcommand for the CLI, or with no arguments to open
//! the GUI.

use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};

use nexus::project::Project;
use nexus::render::parse_time;

#[cfg(feature = "gui")]
mod gui;

#[derive(Parser)]
#[command(name = "nexus", version, about = "A small ffmpeg-backed video editor")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print what ffprobe knows about a media file
    Probe { file: PathBuf },

    /// Trim one file to [start, end) and re-encode
    Trim {
        src: PathBuf,
        #[arg(short, long)]
        start: String,
        /// End time; "0" or omitted means the end of the file
        #[arg(short, long, default_value = "0")]
        end: String,
        #[arg(short, long)]
        out: PathBuf,
    },

    /// Concatenate two or more files into one
    Concat {
        #[arg(required = true, num_args = 2..)]
        files: Vec<PathBuf>,
        #[arg(short, long)]
        out: PathBuf,
        #[arg(long, default_value_t = 1920)]
        width: u32,
        #[arg(long, default_value_t = 1080)]
        height: u32,
        #[arg(long, default_value_t = 30.0)]
        fps: f64,
    },

    /// Create a new, empty project file
    New {
        project: PathBuf,
        #[arg(long, default_value = "Untitled")]
        name: String,
        #[arg(long, default_value_t = 1920)]
        width: u32,
        #[arg(long, default_value_t = 1080)]
        height: u32,
        #[arg(long, default_value_t = 30.0)]
        fps: f64,
    },

    /// Append a clip to a project (out point defaults to the source's end)
    Add {
        project: PathBuf,
        clip: PathBuf,
        #[arg(long = "in", default_value = "0")]
        in_: String,
        #[arg(long)]
        out: Option<String>,
    },

    /// Remove clip N (1-based) from a project
    Rm { project: PathBuf, index: usize },

    /// Print a project's timeline
    Ls { project: PathBuf },

    /// Render a project to a video file
    Export {
        project: PathBuf,
        #[arg(short, long)]
        out: PathBuf,
    },

    /// Open the GUI (also the default with no arguments)
    Gui,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        None | Some(Cmd::Gui) => launch_gui(),
        Some(cmd) => run(cmd),
    }
}

#[cfg(feature = "gui")]
fn launch_gui() -> Result<()> {
    gui::run()
}

#[cfg(not(feature = "gui"))]
fn launch_gui() -> Result<()> {
    bail!("this build has no GUI — run a subcommand instead (`nexus --help`)")
}

/// A progress bar for the terminal.
fn bar() -> impl FnMut(f32, &str) {
    move |p, msg| {
        eprint!("\r\x1b[K{:3.0}%  {}", (p * 100.0).round(), msg);
        if p >= 1.0 {
            eprintln!();
        }
    }
}

fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Probe { file } => {
            let m = nexus::probe(&file)?;
            println!("{}", file.display());
            println!("  duration : {:.3} s", m.duration);
            println!("  size     : {}x{}", m.width, m.height);
            println!("  fps      : {:.3}", m.fps);
            println!("  audio    : {}", if m.has_audio { "yes" } else { "no" });
        }

        Cmd::Trim {
            src,
            start,
            end,
            out,
        } => {
            let mut b = bar();
            nexus::trim(&src, parse_time(&start)?, parse_time(&end)?, &out, &mut b)?;
            println!("wrote {}", out.display());
        }

        Cmd::Concat {
            files,
            out,
            width,
            height,
            fps,
        } => {
            let mut b = bar();
            nexus::concat(&files, &out, width, height, fps, &mut b)?;
            println!("wrote {}", out.display());
        }

        Cmd::New {
            project,
            name,
            width,
            height,
            fps,
        } => {
            if project.exists() {
                bail!("{} already exists", project.display());
            }
            let p = Project {
                name,
                width,
                height,
                fps,
                ..Default::default()
            };
            p.save(&project)?;
            println!("created {}", project.display());
        }

        Cmd::Add {
            project,
            clip,
            in_,
            out,
        } => {
            let mut p = Project::load(&project)?;
            let info = nexus::probe(&clip)?;
            let src_in = parse_time(&in_)?;
            let src_out = match out {
                Some(s) => parse_time(&s)?,
                None => info.duration,
            };
            if src_out <= src_in {
                bail!("out ({src_out}) must be after in ({src_in})");
            }
            p.push_clip(
                std::fs::canonicalize(&clip).unwrap_or(clip),
                src_in,
                src_out,
            );
            p.save(&project)?;
            println!("added clip; timeline is now {:.3} s", p.duration());
        }

        Cmd::Rm { project, index } => {
            let mut p = Project::load(&project)?;
            if index == 0 || index > p.timeline.clips.len() {
                bail!("no clip {index} (timeline has {})", p.timeline.clips.len());
            }
            p.timeline.clips.remove(index - 1);
            p.save(&project)?;
            println!("removed clip {index}");
        }

        Cmd::Ls { project } => {
            let p = Project::load(&project)?;
            println!("{}  ({}x{} @ {:.3} fps)", p.name, p.width, p.height, p.fps);
            if p.timeline.clips.is_empty() {
                println!("  (no clips)");
            }
            let mut t = 0.0;
            for (i, c) in p.timeline.clips.iter().enumerate() {
                println!(
                    "  {:>2}. {}  [{:.3}..{:.3}]  {:.3}s  @ t={:.3}",
                    i + 1,
                    c.source.display(),
                    c.src_in,
                    c.src_out,
                    c.duration(),
                    t
                );
                t += c.duration();
            }
            println!("  total: {:.3} s", p.duration());
        }

        Cmd::Export { project, out } => {
            let p = Project::load(&project)?;
            let mut b = bar();
            nexus::export_project(&p, &out, &mut b)?;
            println!("wrote {}", out.display());
        }

        Cmd::Gui => unreachable!("handled in main"),
    }
    Ok(())
}
