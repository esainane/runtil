use std::env;
use std::process::{Command, exit};

fn print_usage(err: bool) {
    // If it's an error, print to stderr and exit
    if err {
        eprintln!("Usage: runtil <command to poll> [--] <command to run>");
        exit(1);
    }
    // Otherwise, print to stdout
    println!("Usage: runtil <command to poll> [--] <command to run>");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        print_usage(true);
    }

    // TODO: Any options

    // Treat all non-option pieces as belonging to the poll command except for the last piece. If an explicit
    // separator is found, instead all pieces after the separator are part of the run command.
    let mut poll_command = String::new();
    let mut run_command = String::new();
    let mut separator_found = false;

    // Iterate over all arguments except the last one
    for arg in &args[1..args.len()-1] {
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

    println!("Poll command: {}", poll_command);
    println!("Run command: {}", run_command);

    if poll_command.is_empty() || run_command.is_empty() {
        print_usage(true);
    }

    // Fork the run command
    let mut run_command = Command::new("sh")
        .arg("-c")
        .arg(run_command)
        .spawn()
        .expect("Failed to spawn run command");

    // Every 2s, run the poll command
    loop {
        let output = Command::new("sh")
            .arg("-c")
            .arg(&poll_command)
            .output()
            .expect("Failed to spawn poll command");

        if output.status.success() {
            break;
        }

        // Check if the run command has exited
        match run_command.try_wait() {
            Ok(Some(status)) => {
                // Propagate the exit status
                if status.success() {
                    break;
                } else {
                    exit(status.code().unwrap_or(1));
                }
            }
            Ok(None) => {
                // Run command is still running
            }
            Err(e) => {
                eprintln!("Failed to check run command status: {}", e);
                exit(1);
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    // Terminate the run command
    run_command.kill().expect("Failed to kill run command");
}
