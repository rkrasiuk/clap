use std::io::Write;

use clap::builder::StyledStr;
use clap::*;

use crate::generator::{utils, Generator};
use crate::INTERNAL_ERROR_MSG;

/// Generate powershell completion file
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PowerShell;

impl Generator for PowerShell {
    fn file_name(&self, name: &str) -> String {
        format!("_{name}.ps1")
    }

    fn generate(&self, cmd: &Command, buf: &mut dyn Write) {
        let bin_name = cmd
            .get_bin_name()
            .expect("crate::generate should have set the bin_name");

        let subcommands_cases = generate_inner(cmd, "");

        let result = format!(
            r#"
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName '{bin_name}' -ScriptBlock {{
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        '{bin_name}'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {{
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {{
                break
        }}
        $element.Value
    }}) -join ';'

    $completions = @(switch ($command) {{{subcommands_cases}
    }})

    $completions.Where{{ $_.CompletionText -like "$wordToComplete*" }} |
        Sort-Object -Property ListItemText
}}
"#,
            bin_name = bin_name,
            subcommands_cases = subcommands_cases
        );

        w!(buf, result.as_bytes());
    }
}

// Escape string inside single quotes
fn escape_string(string: &str) -> String {
    string.replace('\'', "''")
}

fn get_tooltip<T: ToString>(help: Option<&StyledStr>, data: T) -> String {
    match help {
        Some(help) => escape_string(&help.to_string()),
        _ => data.to_string(),
    }
}

fn generate_inner(p: &Command, previous_command_name: &str) -> String {
    debug!("generate_inner");

    let command_name = if previous_command_name.is_empty() {
        p.get_bin_name().expect(INTERNAL_ERROR_MSG).to_string()
    } else {
        format!("{};{}", previous_command_name, &p.get_name())
    };

    let mut completions = String::new();
    let preamble = String::from("\n            [CompletionResult]::new(");

    for option in p.get_opts() {
        if let Some(shorts) = option.get_short_and_visible_aliases() {
            let tooltip = get_tooltip(option.get_help(), shorts[0]);
            for short in shorts {
                completions.push_str(&preamble);
                completions.push_str(
                    format!(
                        "'-{}', '{}', {}, '{}')",
                        short, short, "[CompletionResultType]::ParameterName", tooltip
                    )
                    .as_str(),
                );
            }
        }

        if let Some(longs) = option.get_long_and_visible_aliases() {
            let tooltip = get_tooltip(option.get_help(), longs[0]);
            for long in longs {
                completions.push_str(&preamble);
                completions.push_str(
                    format!(
                        "'--{}', '{}', {}, '{}')",
                        long, long, "[CompletionResultType]::ParameterName", tooltip
                    )
                    .as_str(),
                );
            }
        }
    }

    for flag in utils::flags(p) {
        if let Some(shorts) = flag.get_short_and_visible_aliases() {
            let tooltip = get_tooltip(flag.get_help(), shorts[0]);
            for short in shorts {
                completions.push_str(&preamble);
                completions.push_str(
                    format!(
                        "'-{}', '{}', {}, '{}')",
                        short, short, "[CompletionResultType]::ParameterName", tooltip
                    )
                    .as_str(),
                );
            }
        }

        if let Some(longs) = flag.get_long_and_visible_aliases() {
            let tooltip = get_tooltip(flag.get_help(), longs[0]);
            for long in longs {
                completions.push_str(&preamble);
                completions.push_str(
                    format!(
                        "'--{}', '{}', {}, '{}')",
                        long, long, "[CompletionResultType]::ParameterName", tooltip
                    )
                    .as_str(),
                );
            }
        }
    }

    for subcommand in p.get_subcommands() {
        let data = &subcommand.get_name();
        let tooltip = get_tooltip(subcommand.get_about(), data);

        completions.push_str(&preamble);
        completions.push_str(
            format!(
                "'{}', '{}', {}, '{}')",
                data, data, "[CompletionResultType]::ParameterValue", tooltip
            )
            .as_str(),
        );
    }

    let mut subcommands_cases = format!(
        r"
        '{}' {{{}
            break
        }}",
        &command_name, completions
    );

    for subcommand in p.get_subcommands() {
        let subcommand_subcommands_cases = generate_inner(subcommand, &command_name);
        subcommands_cases.push_str(&subcommand_subcommands_cases);
    }

    subcommands_cases
}
