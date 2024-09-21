/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use std::io;
use std::process::Command;

use clap::Parser;

use crate::bash::run_bash;
use crate::fish::run_fish;
use crate::zsh::run_zsh;

mod bash;
mod fish;
mod runtime;
mod zsh;

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
#[clap(rename_all = "kebab-case")]
enum Shell {
    Bash,
    Fish,
    Zsh,
}

impl Shell {
    fn find(self) -> io::Result<Command> {
        match self {
            Self::Bash => Ok(Command::new("bash")),
            Self::Fish => {
                let mut path = buck_resources::get("buck2/shed/completion_verify/fish").unwrap();
                path.push("usr/bin/fish");
                Ok(Command::new(path))
            }
            Self::Zsh => {
                if cfg!(target_os = "macos") {
                    Ok(Command::new("zsh"))
                } else {
                    let mut path = buck_resources::get("buck2/shed/completion_verify/zsh").unwrap();
                    path.push("usr/bin/zsh");
                    Ok(Command::new(path))
                }
            }
        }
    }
}

fn extract_from_outputs<S: AsRef<str>>(
    input: &str,
    raw_outs: impl IntoIterator<Item = io::Result<S>>,
) -> io::Result<Vec<String>> {
    for raw_out in raw_outs {
        if let Some(options) = extract_from_single_output(input, raw_out?.as_ref()) {
            return Ok(options);
        }
    }
    Ok(Vec::new())
}

/// Accepts an output like `% buck2 targets` or `% buck2\ntargets   test` and returns
/// the possible completions
fn extract_from_single_output(input: &str, raw_out: &str) -> Option<Vec<String>> {
    if let Some((_, rest)) = raw_out.split_once('\n') {
        // Multiple lines of output indicates there is more than one option. Just naively splitting
        // the output by whitespace is unfortunate wrong in hypothetical cases of completions with
        // spaces, but those should be uncommon so this is fine.
        Some(
            rest.split_ascii_whitespace()
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
        )
    } else {
        let raw_out = raw_out.strip_prefix("% ").unwrap_or(raw_out);

        // No outputed completions
        if raw_out == input || raw_out.is_empty() {
            return None;
        }

        if !raw_out.ends_with(|c: char| c.is_ascii_whitespace()) {
            // Output does not end with whitespace. This means that the output is a partial
            // completion, and so we'll return `None` to indicate that the completion should be
            // retried with an additional tab
            return None;
        }

        // Find the first changed word and copy everything beginning there
        let mut last_equal = 0;
        for (i, c) in raw_out.char_indices() {
            if c.is_ascii_whitespace() && input.len() > i {
                // Include this character in the comparison
                let i = i + 1;
                if raw_out.as_bytes()[..i] == input.as_bytes()[..i] {
                    last_equal = i;
                } else {
                    break;
                }
            }
        }
        Some(vec![raw_out[last_equal..].trim_end().to_owned()])
    }
}

fn run(
    script: &str,
    input: &str,
    tempdir: &Option<String>,
    shell: Shell,
) -> io::Result<Vec<String>> {
    let real_tempdir;
    let tempdir = match tempdir {
        Some(tempdir) => tempdir.as_ref(),
        None => {
            real_tempdir = tempfile::tempdir()?;
            real_tempdir.path()
        }
    };

    match shell {
        Shell::Bash => run_bash(script, input, &tempdir),
        Shell::Fish => run_fish(script, input, &tempdir),
        Shell::Zsh => run_zsh(script, input, &tempdir),
    }
}

/// Helper binary used to test CLI completions.
///
/// Other than the args, it accepts a single line of input containing a partial command invocation
/// to be completed and outputs the possible completions, newline delimited.
#[derive(Debug, clap::Parser)]
#[clap(name = "completion-verify")]
struct CompletionVerify {
    /// The shell to test with
    shell: Shell,
    /// The path of the completion script to load
    script: String,
    /// The path to a directory to use as a tempdir
    ///
    /// Must be empty prior to each invocation of this binary
    tempdir: Option<String>,
}

fn main() -> io::Result<()> {
    let args = CompletionVerify::parse();

    let script = std::fs::read_to_string(&args.script)?;
    let input = std::io::read_to_string(io::stdin())?;

    for option in run(&script, &input, &args.tempdir, args.shell)? {
        println!("{}", option);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::run;
    use crate::Shell;

    const BASH_SCRIPT: &str = "complete -W 'car1 cat2' buck2";

    const FISH_SCRIPT: &str = "complete -c buck2 -a 'car1 cat2'";

    const ZSH_SCRIPT: &str = "\
#compdef buck2
_impl()
{
    compadd car1 cat2
}
compdef _impl buck2
";

    fn test_complete(input: &str, expected: &[&'static str]) {
        check_shell_available(Shell::Bash);
        let actual = run(BASH_SCRIPT, &format!("buck2 {}", input), &None, Shell::Bash).unwrap();
        assert_eq!(actual, expected, "testing bash");

        if cfg!(target_os = "linux") {
            check_shell_available(Shell::Fish);
            let actual = run(FISH_SCRIPT, &format!("buck2 {}", input), &None, Shell::Fish).unwrap();
            assert_eq!(actual, expected, "testing fish");
        }

        check_shell_available(Shell::Zsh);
        let actual = run(ZSH_SCRIPT, &format!("buck2 {}", input), &None, Shell::Zsh).unwrap();
        assert_eq!(actual, expected, "testing zsh");
    }

    fn check_shell_available(shell: Shell) {
        #[allow(clippy::expect_fun_call)]
        let output = shell
            .find()
            .unwrap()
            .arg("--version")
            .output()
            .expect(format!("Failed to run {:?}", shell).as_str());
        assert!(
            output.status.success(),
            "checking that `{:?}` is available",
            shell,
        );
    }

    #[test]
    fn test_zero() {
        test_complete("camp", &[]);
    }

    #[test]
    fn test_one() {
        test_complete("car", &["car1"]);
        test_complete("car1", &["car1"]);
    }

    #[test]
    fn test_two() {
        test_complete("ca", &["car1", "cat2"]);
        test_complete("c", &["car1", "cat2"]);
    }
}