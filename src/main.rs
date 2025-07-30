use anyhow::{Context, Result};
use clap::Parser;
use regex::RegexBuilder;
use std::{collections::HashSet, fs::File, io::{Read, Write}, path::PathBuf};
use walkdir::WalkDir;
use zip::write::FileOptions;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    folder: PathBuf,
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let folder = &args.folder;

    let title = folder
        .file_name()
        .and_then(|s| s.to_str())
        .context("Could not determine folder name as title")?;

    let pattern = format!(r"^{}-(\d+)\.(jpg|jpeg|png)$", regex::escape(title));
    let name_rx = RegexBuilder::new(&pattern)
        .case_insensitive(true)
        .build()?;

    let mut page_entries = Vec::new();
    let mut noise = Vec::new();

    for entry in WalkDir::new(folder).min_depth(1).max_depth(1) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let fname = entry.file_name().to_string_lossy();
        if let Some(caps) = name_rx.captures(&fname) {
            if let Ok(n) = caps[1].parse::<u32>() {
                page_entries.push((n, entry.path().to_path_buf()));
            } else {
                noise.push(fname.into_owned());
            }
        } else {
            noise.push(fname.into_owned());
        }
    }

    if !noise.is_empty() {
        eprintln!("Warning: ignored files not matching pattern:");
        for n in &noise {
            eprintln!("  - {}", n);
        }
    }

    if page_entries.is_empty() {
        anyhow::bail!("No valid image files found matching pattern {}-<number>.<ext>", title);
    }

    page_entries.sort_unstable_by_key(|(n, _)| *n);
    let nums: Vec<u32> = page_entries.iter().map(|(n, _)| *n).collect();
    let max_page = *nums.last().unwrap();
    let num_set: HashSet<u32> = nums.iter().copied().collect();
    let missing: Vec<u32> = (1..=max_page).filter(|i| !num_set.contains(i)).collect();

    if !missing.is_empty() {
        eprintln!("Missing page numbers: {:?}", missing);
        std::process::exit(1);
    }

    let out_path = args
        .output
        .clone()
        .unwrap_or_else(|| folder.with_extension("cbz"));

    let file = File::create(&out_path).context("Failed to create output file")?;
    let mut zip = zip::ZipWriter::new(file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    for (_num, path) in page_entries {
        let mut f = File::open(&path).with_context(|| format!("Failed to open {}", path.display()))?;
        let mut buffer = Vec::new();
        f.read_to_end(&mut buffer).with_context(|| format!("Failed to read {}", path.display()))?;
        let arc_name = path.file_name().unwrap().to_string_lossy();
        zip.start_file(arc_name, options)?;
        zip.write_all(&buffer)?;
    }

    zip.finish().context("Failed to finalize CBZ archive")?;
    println!("Successfully created {}", out_path.display());
    Ok(())
}
