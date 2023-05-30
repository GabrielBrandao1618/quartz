use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "quartz")]
#[command(about = "API Client made into a CLI tool", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Endpoint {
        #[command(subcommand)]
        command: EndpointCommands,
    },
    Layout {
        #[command(subcommand)]
        command: LayoutCommands,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
}

#[derive(Debug, Subcommand)]
pub enum EndpointCommands {
    /// Sends request from endpoint
    Send { endpoint: String },
    Create {
        name: String,

        #[arg(long)]
        url: Option<String>,

        #[arg(long)]
        method: Option<String>,

        /// Header entry in "<key>: <value>" format. This argument can be passed multiple times.
        #[arg(long)]
        header: Vec<String>,
    },
    Headers {
        endpoint: String,

        /// New header entry in "<key>: <value>" format. This argument can be passed multiple times. Overrides duplicates.
        #[arg(long)]
        add: Vec<String>,

        /// Header key to remove from endpoint. This argument can be passed multiple times.
        #[arg(long)]
        remove: Vec<String>,

        /// Prints existing headers.
        #[arg(long)]
        list: bool
    },
}

#[derive(Debug, Subcommand)]
pub enum LayoutCommands {
    /// Creates a new layout.
    Create { name: String },

    /// Creates and switches to a new layout or simply switches to an existing one.
    Use { layout: String },

    /// Prints the current layout name.
    Which,

    /// Lists existing layouts.
    #[command(name = "ls")]
    List,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommands {
    #[command(name = "--edit")]
    Edit,
}
