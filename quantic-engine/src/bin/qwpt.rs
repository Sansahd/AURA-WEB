use std::{collections::BTreeMap, env, fs, path::PathBuf, process};

use quantic_engine::{html, BoaRuntime, JsRuntime};
use serde::Deserialize;
use serde_json::json;

const HARNESS: &str = r#"
globalThis.self = globalThis;
const __q_wpt_results = [];

function __q_wpt_error(message) {
  throw new Error(String(message));
}

function assert_true(value, message = "expected true") {
  if (value !== true) __q_wpt_error(message + ": got " + String(value));
}

function assert_false(value, message = "expected false") {
  if (value !== false) __q_wpt_error(message + ": got " + String(value));
}

function assert_equals(actual, expected, message = "values differ") {
  if (!Object.is(actual, expected)) {
    __q_wpt_error(message + ": expected " + String(expected) + ", got " + String(actual));
  }
}

function assert_not_equals(actual, expected, message = "values unexpectedly equal") {
  if (Object.is(actual, expected)) __q_wpt_error(message + ": " + String(actual));
}

function assert_array_equals(actual, expected, message = "arrays differ") {
  if (!actual || !expected || actual.length !== expected.length) {
    __q_wpt_error(message + ": different lengths");
  }
  for (let index = 0; index < actual.length; index++) {
    if (!Object.is(actual[index], expected[index])) {
      __q_wpt_error(
        message + ": index " + index + " expected " + String(expected[index]) +
        ", got " + String(actual[index])
      );
    }
  }
}

function assert_throws_js(constructor, callback, message = "expected exception") {
  let thrown = null;
  try { callback(); } catch (error) { thrown = error; }
  if (thrown === null) __q_wpt_error(message + ": nothing was thrown");
  if (!(thrown instanceof constructor)) {
    __q_wpt_error(message + ": wrong exception " + String(thrown));
  }
}

function assert_unreached(message = "unreachable code executed") {
  __q_wpt_error(message);
}

function setup() {}
function done() {}

function test(callback, name = "unnamed test") {
  try {
    callback();
    __q_wpt_results.push({ name: String(name), status: "PASS", message: "" });
  } catch (error) {
    __q_wpt_results.push({
      name: String(name),
      status: "FAIL",
      message: String(error && error.message ? error.message : error)
    });
  }
}

globalThis.__q_wpt_summary = () => ({
  passed: __q_wpt_results.filter(result => result.status === "PASS").length,
  failed: __q_wpt_results.filter(result => result.status === "FAIL").length,
  total: __q_wpt_results.length,
  results: __q_wpt_results
});
"#;

#[derive(Debug, Deserialize)]
struct HarnessSummary {
    passed: usize,
    failed: usize,
    total: usize,
    results: Vec<HarnessResult>,
}

#[derive(Debug, Deserialize)]
struct HarnessResult {
    name: String,
    status: String,
    message: String,
}

#[derive(Debug)]
struct FileSummary {
    path: String,
    passed: usize,
    failed: usize,
    total: usize,
    results: Vec<HarnessResult>,
    runtime_error: Option<String>,
}

fn main() {
    let mut min_pass_rate = 0usize;
    let mut files = Vec::<PathBuf>::new();
    let mut arguments = env::args().skip(1);

    while let Some(argument) = arguments.next() {
        if argument == "--min-pass-rate" {
            let Some(value) = arguments.next() else {
                eprintln!("missing value after --min-pass-rate");
                process::exit(2);
            };
            min_pass_rate = value.parse::<usize>().unwrap_or_else(|_| {
                eprintln!("invalid --min-pass-rate: {value}");
                process::exit(2);
            });
            if min_pass_rate > 100 {
                eprintln!("--min-pass-rate must be between 0 and 100");
                process::exit(2);
            }
        } else {
            files.push(PathBuf::from(argument));
        }
    }

    if files.is_empty() {
        eprintln!("usage: qwpt [--min-pass-rate PERCENT] <wpt-file.js>...");
        process::exit(2);
    }

    let mut summaries = Vec::new();
    for path in files {
        summaries.push(run_file(&path));
    }

    let passed = summaries
        .iter()
        .map(|summary| summary.passed)
        .sum::<usize>();
    let failed = summaries
        .iter()
        .map(|summary| summary.failed)
        .sum::<usize>();
    let total = summaries.iter().map(|summary| summary.total).sum::<usize>();
    let runtime_errors = summaries
        .iter()
        .filter(|summary| summary.runtime_error.is_some())
        .count();
    let pass_rate = passed.saturating_mul(100).checked_div(total).unwrap_or(0);

    let files_json = summaries
        .iter()
        .map(|summary| {
            json!({
                "path": summary.path,
                "passed": summary.passed,
                "failed": summary.failed,
                "total": summary.total,
                "runtime_error": summary.runtime_error,
                "results": summary.results.iter().map(|result| json!({
                    "name": result.name,
                    "status": result.status,
                    "message": result.message,
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "runner": "Quantic WPT probe",
            "passed": passed,
            "failed": failed,
            "total": total,
            "runtime_errors": runtime_errors,
            "pass_rate_percent": pass_rate,
            "minimum_pass_rate_percent": min_pass_rate,
            "files": files_json,
        }))
        .expect("WPT summary JSON")
    );

    if runtime_errors > 0 || total == 0 || pass_rate < min_pass_rate {
        process::exit(1);
    }
}

fn run_file(path: &PathBuf) -> FileSummary {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            return FileSummary {
                path: path.display().to_string(),
                passed: 0,
                failed: 1,
                total: 1,
                results: Vec::new(),
                runtime_error: Some(error.to_string()),
            };
        }
    };

    let document = html::parse("<html><body></body></html>");
    let mut runtime = match BoaRuntime::new(&document, &BTreeMap::new()) {
        Ok(runtime) => runtime,
        Err(error) => {
            return FileSummary {
                path: path.display().to_string(),
                passed: 0,
                failed: 1,
                total: 1,
                results: Vec::new(),
                runtime_error: Some(error.to_string()),
            };
        }
    };

    let result = runtime
        .eval_script(HARNESS)
        .and_then(|_| runtime.eval_script(&source))
        .and_then(|_| {
            runtime.eval_script("console.log('__QWPT__' + JSON.stringify(__q_wpt_summary()));")
        })
        .and_then(|_| runtime.drain_effects());

    match result {
        Ok(effects) => {
            let payload = effects
                .console
                .iter()
                .rev()
                .find_map(|message| message.message.strip_prefix("__QWPT__"));
            let Some(payload) = payload else {
                return FileSummary {
                    path: path.display().to_string(),
                    passed: 0,
                    failed: 1,
                    total: 1,
                    results: Vec::new(),
                    runtime_error: Some("WPT harness produced no summary".to_string()),
                };
            };
            match serde_json::from_str::<HarnessSummary>(payload) {
                Ok(summary) => FileSummary {
                    path: path.display().to_string(),
                    passed: summary.passed,
                    failed: summary.failed,
                    total: summary.total,
                    results: summary.results,
                    runtime_error: None,
                },
                Err(error) => FileSummary {
                    path: path.display().to_string(),
                    passed: 0,
                    failed: 1,
                    total: 1,
                    results: Vec::new(),
                    runtime_error: Some(format!("invalid WPT summary: {error}")),
                },
            }
        }
        Err(error) => FileSummary {
            path: path.display().to_string(),
            passed: 0,
            failed: 1,
            total: 1,
            results: Vec::new(),
            runtime_error: Some(error.to_string()),
        },
    }
}
