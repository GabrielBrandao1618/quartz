use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "quartz")]
#[command(author = "Eduardo R. <contato@edurodrigues.dev>")]
#[command(about = "Text-based API Client", long_about = None, version)]
pub struct Cli {
    /// Run quartz using a specific handle
    #[arg(short = 'x', value_name = "HANDLE")]
    pub from_handle: Option<String>,

    /// Apply context on endpoint as soon as possible. Allows to get resolved information on
    /// output
    #[arg(short = 'c', long)]
    pub apply_context: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Initialize quartz
    Init { directory: Option<PathBuf> },
    /// Send request using the current handle's endpoint and outputs the response
    Send {
        /// Which fields to show after the request is complete, separated by comma (,). See manual page for a list of valid fields
        #[arg(long, short, value_delimiter = ',', value_name = "FIELDS")]
        show: Vec<String>,

        /// Change a variable when sending the request.
        #[arg(long, value_name = "VARIABLE")]
        var: Vec<String>,

        /// Change or include an extra header
        #[arg(long, short = 'H')]
        header: Vec<String>,

        /// Change or include an extra query param
        #[arg(long)]
        query: Vec<String>,

        /// Change request method
        #[arg(long, short = 'X', value_name = "METHOD")]
        request: Option<String>,

        /// Sends data in request body
        #[arg(long, short = 'd')]
        data: Option<String>,
    },
    /// Create a new handle
    Create {
        handle: String,

        /// Set handle's endpoint URL
        #[arg(long)]
        url: Option<String>,

        /// Set handle's endpoint method value
        #[arg(short = 'X', long = "request")]
        method: Option<String>,

        /// Add a key-value pair to the URL query.
        #[arg(short, long, value_name = "PARAM")]
        query: Vec<String>,

        /// Set a header entry in "<key>: <value>" format. This argument can be passed multiple times
        #[arg(short = 'H', long)]
        header: Vec<String>,

        /// Immediatly switches to this handle after creating it.
        #[arg(name = "use", long)]
        switch: bool,
    },
    /// Switch handle or edit its endpoint
    Use {
        handle: Option<String>,

        #[arg(long)]
        url: Option<String>,

        /// HTTP request method
        #[arg(short = 'X', long = "request")]
        method: Option<String>,

        /// Add a parameter the URL query.
        #[arg(short, long, value_name = "PARAM")]
        query: Vec<String>,

        /// Add a header entry in "<key>: <value>" format. This argument can be passed multiple times
        #[arg(short = 'H', long)]
        header: Vec<String>,
    },
    /// Print the current status of quartz
    Status {
        #[command(subcommand)]
        command: StatusCommands,
    },
    /// Lists available handles
    #[command(name = "ls", alias = "list")]
    List {
        /// Set a limit for how deep the listing goes in sub-handles
        #[arg(long, value_name = "N")]
        depth: Option<u16>,
    },
    /// Delete the specified handle recursively
    #[command(name = "rm", alias = "remove")]
    Remove {
        /// Endpoint specification
        handle: String,
    },
    /// Print out endpoint informations
    Show {
        #[command(subcommand)]
        command: Option<EndpointShowCommands>,
    },
    /// Open an editor to modify endpoint in use
    Edit {
        #[arg(long)]
        /// Defines the editor to be used for that run, overriding the quartz settings.
        editor: Option<String>,
    },
    /// Manage current handle's endpoint query params
    Query {
        #[command(subcommand)]
        command: EndpointQueryCommands,
    },
    /// Manage current handle's endpoint headers
    Header {
        /// Add new header entry in "key: value" format
        #[arg(long, value_name = "HEADER")]
        set: Vec<String>,

        /// Print a header value
        #[arg(long, value_name = "KEY")]
        get: Option<String>,

        /// Remove a header
        #[arg(long, value_name = "KEY")]
        remove: Vec<String>,

        /// Print headers
        #[arg(long)]
        list: bool,
    },
    /// Manage current handle's endpoint request body
    Body {
        /// Expect a new request body via standard input
        #[arg(long)]
        stdin: bool,

        /// Open an editor to modify the endpoint's request body
        #[arg(long, short)]
        edit: bool,

        /// Print request body
        #[arg(long, short)]
        print: bool,
    },
    /// Print information about last request or response
    Last {
        /// Format date time output
        #[arg(long, value_name = "FORMAT")]
        date: Option<String>,

        #[command(subcommand)]
        command: Option<LastCommands>,
    },
    /// Print request history
    History {
        /// Maximum number of requests to be listed
        #[arg(short = 'n', long, value_name = "N")]
        max_count: Option<usize>,
        /// Format date time output
        #[arg(long, value_name = "FORMAT")]
        date: Option<String>,

        /// Which fields to show, separated by comma (,). See manual page for a list of valid fields
        #[arg(long, short, value_delimiter = ',', value_name = "FIELDS")]
        show: Vec<String>,
    },
    #[command(name = "ctx", alias = "context")]
    Context {
        #[command(subcommand)]
        command: ContextCommands,
    },
    /// Manage current context's variables
    #[command(name = "var", alias = "variable")]
    Variable {
        /// Print a variable value
        #[arg(long, value_name = "KEY")]
        get: Option<String>,

        /// Set a variable: key=value
        #[arg(long, value_name = "VARIABLE")]
        set: Vec<String>,

        /// Print all variables
        #[arg(long)]
        list: bool,

        /// Open an editor to modify the context variables file
        #[arg(short, long)]
        edit: bool,
    },
    /// Manage configuration for quartz
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
}

#[derive(Debug, Subcommand)]
pub enum StatusCommands {
    /// Print the handle for the endpoint in use
    #[command(name = "--endpoint")]
    Endpoint,

    /// Print the context in use
    #[command(name = "--context")]
    Context,
}

#[derive(Debug, Subcommand)]
pub enum LastCommands {
    /// Print most recent handle used
    Handle,

    /// Print most recent request date
    Date,

    /// Print last request information
    Request {
        #[command(subcommand)]
        command: LastRequestCommands,
    },
    /// Print last response information
    Response {
        #[command(subcommand)]
        command: LastResponseCommands,
    },
}

#[derive(Debug, Subcommand)]
pub enum LastRequestCommands {
    Url,
    Query,
    Method,
    Headers,
    Body,
    Context,
}

#[derive(Debug, Subcommand)]
pub enum LastResponseCommands {
    Status,
    Headers,
    Body,
    Size,
}

#[derive(Debug, Subcommand)]
pub enum EndpointQueryCommands {
    /// Print query param value
    Get { key: Option<String> },

    /// Set query param value
    Set { query: Vec<String> },

    /// Remove query param
    #[command(name = "rm", alias = "remove")]
    Remove { key: String },

    /// List all query params
    #[command(name = "ls", alias = "list")]
    List,
}

#[derive(Debug, Subcommand)]
pub enum EndpointShowCommands {
    Url,
    Method,
    Headers { key: Option<String> },
    Query { key: Option<String> },
    Body,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommands {
    /// Open an editor to modify ~/.quartz.toml
    Edit,

    /// Print configuration value
    Get { key: String },

    /// Set a configuration
    Set { key: String, value: String },

    /// Print ~/.quartz.toml
    #[command(name = "ls", alias = "list")]
    List,
}

#[derive(Debug, Subcommand)]
pub enum ContextCommands {
    /// Create a new context
    Create { name: String },
    /// Switch to another context
    Use { context: String },
    /// Print all available contexts
    #[command(name = "ls", alias = "list")]
    List,

    /// Copy variables from a context to a new or existing one
    #[command(name = "cp", alias = "copy")]
    Copy { src: String, dest: String },
    /// Delete a context
    #[command(name = "rm", alias = "remove")]
    Remove { context: String },
}
