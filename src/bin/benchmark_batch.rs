use eva_runtime_with_task_validator::{
    BenchmarkBatchReport, BenchmarkCaseLoader, BenchmarkRunner, DEFAULT_BATCH_REPORT_PATH,
};

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let manifest_path = args
        .get(0)
        .cloned()
        .unwrap_or_else(|| "benchmarks/rust_cases_ready.json".to_string());
    let output_path = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| DEFAULT_BATCH_REPORT_PATH.to_string());
    let limit = args.get(2).and_then(|value| value.parse::<usize>().ok());

    let result = run(&manifest_path, &output_path, limit);
    match result {
        Ok(report) => println!(
            "{}",
            serde_json::to_string_pretty(&report.aggregate).expect("serialize aggregate")
        ),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn run(
    manifest_path: &str,
    output_path: &str,
    limit: Option<usize>,
) -> Result<BenchmarkBatchReport, String> {
    let manifest = BenchmarkCaseLoader::load_manifest(manifest_path)?;
    let report = BenchmarkRunner::default().run_manifest(&manifest, limit)?;
    report.write_to_path(output_path)?;
    Ok(report)
}
