use clap::builder::{PossibleValuesParser, TypedValueParser, ValueParser};
use std::{io::stdin, ops::Deref};

pub mod account;
pub mod api_access;
pub mod auto_connect;
pub mod beta_program;
pub mod bridge;
pub mod custom_list;
pub mod debug;
pub mod dns;
pub mod lan;
pub mod lockdown;
pub mod obfuscation;
pub mod patch;
pub mod proxies;
pub mod relay;
pub mod relay_constraints;
pub mod reset;
pub mod split_tunnel;
pub mod status;
pub mod tunnel;
pub mod tunnel_state;
pub mod version;

/// A value parser that parses "on" or "off" into a boolean
#[derive(Debug, Clone, Copy)]
pub struct BooleanOption {
    state: bool,
    on_label: &'static str,
    off_label: &'static str,
}

impl Deref for BooleanOption {
    type Target = bool;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl clap::builder::ValueParserFactory for BooleanOption {
    type Parser = ValueParser;

    /// A value parser that parses "on" or "off" into a `BooleanOption`
    fn value_parser() -> Self::Parser {
        Self::custom_parser("on", "off")
    }
}

impl BooleanOption {
    /// A value parser that parses `on_label` and `off_label` into a `BooleanOption`
    fn custom_parser(on_label: &'static str, off_label: &'static str) -> ValueParser {
        assert!(on_label != off_label);

        ValueParser::new(
            PossibleValuesParser::new([on_label, off_label])
                .map(move |val| Self::with_labels(val == on_label, on_label, off_label)),
        )
    }

    fn with_labels(state: bool, on_label: &'static str, off_label: &'static str) -> Self {
        Self {
            state,
            on_label,
            off_label,
        }
    }
}

impl From<bool> for BooleanOption {
    fn from(state: bool) -> Self {
        Self::with_labels(state, "on", "off")
    }
}

impl std::fmt::Display for BooleanOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.state {
            self.on_label.fmt(f)
        } else {
            self.off_label.fmt(f)
        }
    }
}

async fn receive_confirmation(msg: &'static str, default: bool) -> bool {
    let helper_str = match default {
        true => "[Y/n]",
        false => "[y/N]",
    };

    println!("{msg} {helper_str}");

    tokio::task::spawn_blocking(move || {
        loop {
            let mut buf = String::new();
            if let Err(e) = stdin().read_line(&mut buf) {
                eprintln!("Couldn't read from STDIN: {e}");
                return false;
            }
            match buf.trim().to_ascii_lowercase().as_str() {
                "" => return default,
                "y" | "ye" | "yes" => return true,
                "n" | "no" => return false,
                _ => eprintln!("Unexpected response. Please enter \"y\" or \"n\""),
            }
        }
    })
    .await
    .unwrap()
}
