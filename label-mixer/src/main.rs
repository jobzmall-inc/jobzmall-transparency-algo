use chrono::DateTime;
use clap::Parser;
use jm_enforcement::recommend;
use jm_engine::compile;
use jm_params::Params;
use label_mixer::load_listing;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(about = "JobzMall Label Compiler — warranted judgment + Nutrition Label")]
struct Args {
    /// A listing JSON file, or used with --dir
    path: Option<PathBuf>,
    /// Score every *.json in a directory
    #[arg(long)]
    dir: Option<PathBuf>,
    /// Optional local policy JSON (real fraud thresholds, etc.)
    #[arg(long)]
    policy: Option<PathBuf>,
    /// Override the evaluation instant (RFC3339). Default: derived from the listing.
    #[arg(long)]
    now: Option<String>,
}

fn main() {
    let args = Args::parse();
    let params = match args.policy {
        Some(path) => Params::from_policy_file(&path).unwrap_or_else(|e| {
            eprintln!("policy: {e}");
            std::process::exit(2);
        }),
        None => {
            let p = Params::default();
            if let Err(e) = p.validate() {
                eprintln!("policy: {e}");
                std::process::exit(2);
            }
            p
        }
    };

    let now = args.now.as_deref().map(|s| {
        DateTime::parse_from_rfc3339(s)
            .unwrap_or_else(|e| {
                eprintln!("--now: {e}");
                std::process::exit(2);
            })
            .with_timezone(&chrono::Utc)
    });

    let mut dossiers = Vec::new();

    if let Some(dir) = args.dir {
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1);
            })
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
            .collect();
        files.sort();
        for path in files {
            match load_listing(&path) {
                Ok(listing) => dossiers.push(compile(listing, &params, now)),
                Err(e) => eprintln!("{}: {e}", path.display()),
            }
        }
    } else if let Some(path) = args.path {
        match load_listing(&path) {
            Ok(listing) => dossiers.push(compile(listing, &params, now)),
            Err(e) => {
                eprintln!("{}: {e}", path.display());
                std::process::exit(1);
            }
        }
    } else {
        eprintln!("pass a listing JSON or --dir");
        std::process::exit(2);
    }

    if dossiers.len() == 1 {
        let dossier = &dossiers[0];
        println!("{}", serde_json::to_string_pretty(dossier).unwrap());
        println!("\n# enforcement recommendation: {:?}", recommend(dossier));
        if let Some(c) = dossier.aftermath.correction {
            println!("# aftermath correction: {c:?}");
        }
    } else {
        let report = jm_public_report::aggregate(&dossiers);
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        println!("\n# {} dossiers", dossiers.len());
        for dossier in &dossiers {
            println!(
                "- {} · {} · {:?} · {} · {} facts / {} claims",
                dossier.slug,
                dossier.source.kind.as_label(),
                dossier.disposition,
                dossier.decided_by,
                dossier.facts_extracted,
                dossier.claims_surviving
            );
        }
    }
}
