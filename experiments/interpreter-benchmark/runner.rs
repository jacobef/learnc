use c_interpreter::{NativeExecutionOptions, run_native_source};
use std::{env, fs, time::Instant};

fn main() {
    let args: Vec<_> = env::args().collect();
    let source = fs::read_to_string(&args[1]).unwrap();
    let options = NativeExecutionOptions {
        stdin: args[2].clone(),
        execution_step_limit: Some(2_000_000),
        ..Default::default()
    };
    if let Ok(seconds) = env::var("PROFILE_SECONDS") {
        let duration = std::time::Duration::from_secs(seconds.parse().unwrap());
        let start = Instant::now();
        while start.elapsed() < duration {
            std::hint::black_box(run_native_source("benchmark.c", &source, &options).unwrap());
        }
        return;
    }
    let mut samples = Vec::new();
    let mut output = String::new();
    for trial in 0..9 {
        let start = Instant::now();
        let result = run_native_source("benchmark.c", &source, &options).unwrap();
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(result.exit_status, 0);
        assert!(result.stderr.is_empty());
        output = result.stdout;
        if trial >= 2 { samples.push(ms); }
    }
    println!("{{\"output\":{},\"ms\":{:?}}}", output.trim(), samples);
}
