use std::env;
use std::process::exit;
// XXX: Maybe this should keep the Command name, instead of making TokioCommand explicit?
use tokio::process::Command as TokioCommand;
use tokio_util::sync::CancellationToken;
use tokio::time::{sleep, Duration, Instant};
use tokio::pin;

struct Config {
    verbose: bool,
    kill_condition_code: i32,
}

fn print_usage(err: bool) {
    // If it's an error, print to stderr and exit
    if err {
        eprintln!("Usage: runtil [options] <command to poll> [--] <command to run>");
        exit(1);
    }
    // Otherwise, print to stdout
    println!("Usage: runtil [options] <command to poll> [--] <command to run>");
}

fn parse_options(args: &[String]) -> (usize, Config) {
    let mut verbose = false;
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "-v" => verbose = true,
            _ => break,
        }
        index += 1;
    }

    (index, Config { verbose, kill_condition_code: 124 })
}

fn parse_arguments(args: &[String]) -> (String, String, Config) {
    let (args_consumed_index, config) = parse_options(args);

    // Treat all non-option pieces as belonging to the poll command except for the last piece. If an explicit
    // separator is found, instead all pieces after the separator are part of the run command.
    let mut poll_command = String::new();
    let mut run_command = String::new();
    let mut separator_found = false;

    // Iterate over all arguments except the last one
    for arg in &args[args_consumed_index..args.len() - 1] {
        if arg == "--" {
            separator_found = true;
        } else if separator_found {
            if !run_command.is_empty() {
                run_command.push(' ');
            }
            run_command.push_str(arg);
        } else {
            if !poll_command.is_empty() {
                poll_command.push(' ');
            }
            poll_command.push_str(arg);
        }
    }

    // Add the last argument
    if !run_command.is_empty() {
        run_command.push(' ');
    }
    run_command.push_str(&args[args.len() - 1]);

    (poll_command, run_command, config)
}

async fn run_task(run_command: String, token: CancellationToken, config: &Config) -> i32 {
    let mut run_task = TokioCommand::new("sh")
        .arg("-c")
        .arg(run_command)
        .spawn()
        .expect("Failed to spawn run command");

    tokio::select! {
        _ = token.cancelled() => {
            // Cancel the run command
            run_task.kill().await.expect("Failed to kill run command");
            return config.kill_condition_code;
        },
        status = run_task.wait() => {
            // Return the exit code
            match status {
                Ok(status) => return status.code().unwrap_or(1),
                Err(e) => {
                    eprintln!("Failed to wait for run command: {}", e);
                    exit(1);
                }
            }
        }
    }
}

async fn run_conditional(poll_command: String, token: CancellationToken, _config: &Config) {
    loop {
        let start_time = Instant::now();

        let poll_task = TokioCommand::new("sh")
            .arg("-c")
            .arg(&poll_command)
            .output();

        // Wait for a result from the poll command, or for us to be cancelled
        tokio::select! {
            status = poll_task => if status.expect("Failed to spawn conditional command").status.success() {
                return;
            },
            _ = token.cancelled() => return,
        }

        // Wait up to 2s before polling again
        let elapsed = start_time.elapsed();
        if elapsed < Duration::from_secs(2) {
            sleep(Duration::from_secs(2) - elapsed).await;
        }
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        print_usage(true);
    }

    let (poll_command, run_command, config) = parse_arguments(&args);

    if config.verbose {
        println!("Poll command: {}", poll_command);
        println!("Run command: {}", run_command);
    }

    if poll_command.is_empty() || run_command.is_empty() {
        print_usage(true);
    }

    // Create tokens to cancel tasks when required
    let cancel_task = CancellationToken::new();
    let cancel_conditional = CancellationToken::new();

    // Run the task and the conditional command concurrently
    let task_result = run_task(run_command, cancel_task.clone(), &config);
    let conditional = run_conditional(poll_command, cancel_conditional.clone(), &config);

    // Use the result in both branches
    pin!(task_result);

    // Wait for either the run command to complete, or the conditional command to succeed
    tokio::select! {
        code = &mut task_result => {
            // Run command completed.

            // Kill the conditional command
            cancel_conditional.cancel();

            // Exit, propagating the exit code
            exit(code);
        },
        _ = conditional => {
            // Poll command succeeded, kill the run command
            cancel_task.cancel();
            exit(task_result.await);
        }
    };
}
