use std::env;
use std::process::{exit};
// XXX: Maybe this should keep the Command name, instead of making TokioCommand explicit?
use tokio::process::Command as TokioCommand;
use tokio::time::{sleep, Duration, Instant};

struct Config {
    verbose: bool,
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

    (index, Config { verbose })
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        print_usage(true);
    }

    let (args_consumed_index, config) = parse_options(&args);

    // Treat all non-option pieces as belonging to the poll command except for the last piece. If an explicit
    // separator is found, instead all pieces after the separator are part of the run command.
    let mut poll_command = String::new();
    let mut run_command = String::new();
    let mut separator_found = false;

    // Iterate over all arguments except the last one
    for arg in &args[args_consumed_index..args.len()-1] {
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

    if config.verbose {
        println!("Poll command: {}", poll_command);
        println!("Run command: {}", run_command);
    }

    if poll_command.is_empty() || run_command.is_empty() {
        print_usage(true);
    }

    // Fork the run command
    let mut run_command = TokioCommand::new("sh")
        .arg("-c")
        .arg(run_command)
        .spawn()
        .expect("Failed to spawn run command");

    let poll_command = poll_command.clone();

    // Spawn a task to run the poll command every 2 seconds
    let poll_task = tokio::spawn(async move {
        loop {
            let start_time = Instant::now();

            let output = TokioCommand::new("sh")
                .arg("-c")
                .arg(&poll_command)
                .output()
                .await
                .expect("Failed to spawn poll command");

            if output.status.success() {
                break;
            }

            let elapsed = start_time.elapsed();
            if elapsed < Duration::from_secs(2) {
                sleep(Duration::from_secs(2) - elapsed).await;
            }
        }
    });

    // Wait for either the run command to exit or the poll command to succeed
    tokio::select! {
        status = run_command.wait() => {
            match status {
                Ok(status) => {
                    if !status.success() {
                        exit(status.code().unwrap_or(1));
                    }
                }
                Err(e) => {
                    eprintln!("Failed to wait for run command: {}", e);
                    exit(1);
                }
            }
        }
        _ = poll_task => {
            // Poll command succeeded, kill the run command
            run_command.kill().await.expect("Failed to kill run command");
        }
    }
}
