use std::env;
use std::process::ExitCode;

use okf::{BundleParser, SearchQuery, Validator};

fn main() -> ExitCode {
    let Some(root) = env::args_os().nth(1) else {
        eprintln!("usage: cargo run --example inspect -- <bundle-directory> [query]");
        return ExitCode::from(2);
    };
    let query = env::args().nth(2);

    let bundle = match BundleParser::default().parse_dir(root) {
        Ok(bundle) => bundle,
        Err(error) => {
            eprintln!("failed to load bundle: {error}");
            return ExitCode::FAILURE;
        }
    };

    println!(
        "loaded {} documents from {}",
        bundle.len(),
        bundle.root().display()
    );
    let report = Validator::default().validate(&bundle);
    for issue in report.issues() {
        let document = issue
            .document
            .as_ref()
            .map_or_else(String::new, |id| format!(" [{id}]"));
        println!(
            "{:?} {}{}: {}",
            issue.severity, issue.code, document, issue.message
        );
    }

    if let Some(query) = query {
        for hit in bundle.search(&SearchQuery::new(query).limit(10)) {
            println!(
                "{}\t{}\t{}",
                hit.score,
                hit.document.id(),
                hit.document.title()
            );
        }
    }

    if report.is_valid() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
