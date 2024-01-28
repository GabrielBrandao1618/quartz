use std::{
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::Parser;
use colored::Colorize;
use hyper::{
    body::{Bytes, HttpBody},
    Body, Client,
};
use tokio::io::{stdout, AsyncWriteExt as _};
use tokio::time::Instant;

use quartz_cli::{
    cli::EndpointShowCommands,
    history::{self, History, HistoryEntry},
};
use quartz_cli::{
    cli::{self, Cli, Commands},
    config::Config,
    endpoint::{Endpoint, EndpointHandle, Headers},
    Ctx, CtxArgs, PairMap,
};
use quartz_cli::{context::Context, endpoint::EndpointInput};
use quartz_cli::{state::StateField, validator};

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    let mut ctx = Ctx::new(CtxArgs {
        from_handle: args.from_handle,
        early_apply_context: args.apply_context,
    });

    // When true, ensures pagers and/or grep keeps the output colored
    colored::control::set_override(ctx.config.ui.colors);

    std::panic::set_hook(Box::new(|info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else {
            info.to_string()
        };

        eprintln!("{}: {payload}", "error".red().bold());
    }));

    match args.command {
        Commands::Init { directory } => {
            let directory = directory.unwrap_or(Path::new(".").to_path_buf());
            let quartz_dir = directory.join(".quartz");

            if quartz_dir.exists() {
                panic!(
                    "quartz already initialized at {}",
                    directory.to_string_lossy()
                );
            }

            if std::fs::create_dir(&quartz_dir).is_err() {
                panic!("failed to initialize quartz project");
            };

            let ensure_dirs = vec![
                "endpoints",
                "user",
                "user/history",
                "user/state",
                "contexts",
            ];

            for dir in ensure_dirs {
                if std::fs::create_dir(quartz_dir.join(PathBuf::from_str(dir).unwrap())).is_err() {
                    panic!("failed to create {} directory", dir);
                }
            }

            if directory.join(".git").exists() {
                println!("Git detected");
                println!("Adding user files to {}", ".gitignore".green());

                if let Ok(mut gitignore) = std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .append(true)
                    .open(directory.join(".gitignore"))
                {
                    let _ = gitignore.write("\n# Quartz\n.quartz/user".as_bytes());
                }
            }

            if Context::default().write().is_err() {
                panic!("failed to create default context");
            }

            ctx.config
                .write()
                .unwrap_or_else(|_| panic!("failed to save configuration file"));
        }
        Commands::Send {
            show: show_fields,
            header: headers,
            query,
            var: variables,
            request,
            data,
        } => {
            let (specification, mut endpoint) = ctx.require_endpoint();
            let mut context = ctx.require_context();

            for var in variables {
                context.variables.set(&var);
            }

            endpoint.update(&mut EndpointInput {
                headers,
                query,
                method: request,
                ..Default::default()
            });

            endpoint.apply_context(&context);

            let raw_body = match data {
                Some(data) => Body::from(data),
                None => endpoint.body(&specification),
            };

            let req = endpoint
                // TODO: Find a way around this clone
                .clone()
                .into_request(&specification)
                .unwrap_or_else(|_| panic!("malformed request"));

            let client = {
                let https = hyper_tls::HttpsConnector::new();
                Client::builder().build(https)
            };

            let start = Instant::now();
            let mut res = client.request(req).await.unwrap();
            let duration = start.elapsed().as_millis() as u64;
            let status = res.status().as_u16();

            let mut bytes = Bytes::new();
            let mut size = 0;

            while let Some(chunk) = res.data().await {
                if let Ok(chunk) = chunk {
                    size = chunk.len();
                    bytes = chunk;
                }
            }

            let entry: HistoryEntry = {
                let mut headers = Headers::default();
                for (key, value) in res.headers() {
                    headers.insert(key.to_string(), String::from(value.to_str().unwrap_or("")));
                }

                let req_body_bytes = hyper::body::to_bytes(raw_body).await.unwrap();

                let request = history::Request {
                    endpoint,
                    context,
                    duration,
                    body: String::from_utf8_lossy(&req_body_bytes).to_string(),
                };
                let response = history::Response {
                    status,
                    size,
                    body: String::from_utf8_lossy(&bytes).to_string(),
                    headers,
                };

                HistoryEntry::new(specification.handle(), request, response)
            };

            if show_fields.is_empty() {
                // Regular output
                println!("Status: {}", res.status());
                println!("Duration: {}ms", duration);
                println!("Size: {} bytes", size);

                let _ = stdout().write_all(&bytes).await;
            } else {
                let mut outputs: Vec<String> = Vec::new();
                for key in &show_fields {
                    match entry.field_as_string(key) {
                        Ok(value) => outputs.push(value),
                        Err(..) => eprintln!("invalid field: {}", key),
                    }
                }

                for value in outputs {
                    println!("{}", value);
                }

                return;
            }

            let _ = entry.write();
        }
        Commands::Create {
            handle,
            url,
            method,
            query,
            header: headers,
            switch,
        } => {
            if handle.is_empty() {
                panic!("missing endpoint handle");
            }

            let handle = EndpointHandle::from_handle(handle);

            if handle.exists() {
                panic!("endpoint already exists");
            }

            let mut endpoint = Endpoint::from(&mut EndpointInput {
                url,
                method,
                query,
                headers,
                ..Default::default()
            });

            if switch {
                if let Ok(()) = StateField::Endpoint.set(&handle.path.join("/")) {
                    println!("Switched to {} endpoint", handle.handle().green());
                } else {
                    panic!("failed to switch to {} endpoint", handle.handle().red());
                }
            }

            handle.write();
            endpoint.write(&handle);
        }
        Commands::Use {
            handle,
            url,
            method,
            query,
            header: headers,
        } => {
            let (handle, mut endpoint) = if let Some(handle) = handle {
                let handle = ctx.require_input_handle(&handle);

                if !handle.dir().exists() {
                    panic!("endpoint does not exist");
                }

                if let Ok(()) = StateField::Endpoint.set(&handle.path.join("/")) {
                    println!("Switched to {} endpoint", handle.handle().green());
                } else {
                    panic!("failed to switch to {} endpoint", handle.handle().red());
                }

                let endpoint = ctx.require_endpoint_from_handle(&handle);
                (handle, endpoint)
            } else {
                ctx.require_endpoint()
            };

            endpoint.update(&mut EndpointInput {
                url,
                method,
                query,
                headers,
                ..Default::default()
            });
            endpoint.write(&handle);
        }
        Commands::Status { command } => match command {
            cli::StatusCommands::Endpoint => {
                if let Ok(endpoint) = ctx.state.get(StateField::Endpoint) {
                    println!("{}", endpoint);
                }
            }
            cli::StatusCommands::Context => {
                println!(
                    "{}",
                    ctx.state
                        .get(StateField::Context)
                        .unwrap_or("default".into())
                );
            }
        },
        Commands::List { depth: max_depth } => {
            let max_depth = max_depth.unwrap_or(999) as i16;
            let mut current = PathBuf::new();

            if let Some(specification) = EndpointHandle::from_state(&ctx.state) {
                current = specification.dir()
            }

            // This code is a mess.
            // I'm sorry.
            // It will be refactored sometime.
            struct TraverseEndpoints<'s> {
                f: &'s dyn Fn(&TraverseEndpoints, Vec<EndpointHandle>),
            }
            let traverse_handles = TraverseEndpoints {
                f: &|recurse, specifications| {
                    for spec in specifications {
                        let depth = (spec.path.len() as i16 - 1).max(0);
                        let children = spec.children();

                        if let Some(endpoint) = spec.endpoint() {
                            if current == spec.dir() {
                                print!(
                                    "*  {: >5} {}",
                                    endpoint.colored_method().bold(),
                                    spec.handle().green()
                                );
                            } else {
                                print!(
                                    "   {: >5} {}",
                                    endpoint.colored_method().bold(),
                                    spec.handle()
                                );
                            }
                        } else if !spec.path.is_empty() {
                            print!("   {: >5} {}", "---".dimmed(), spec.handle());
                        }

                        if !children.is_empty() {
                            if depth < max_depth {
                                // Avoid extra newline from Specification::QUARTZ usage
                                if !spec.path.is_empty() {
                                    println!();
                                }

                                (recurse.f)(recurse, children);
                            } else {
                                println!("{}", " +".dimmed());
                            }
                        } else {
                            println!();
                        }
                    }
                },
            };

            (traverse_handles.f)(&traverse_handles, vec![EndpointHandle::QUARTZ]);
        }
        Commands::Show { command } => {
            let (handle, endpoint) = ctx.require_endpoint();

            if let Some(command) = command {
                match command {
                    EndpointShowCommands::Url => println!("{}", endpoint.url),
                    EndpointShowCommands::Method => println!("{}", endpoint.method),
                    EndpointShowCommands::Query { key } => {
                        if let Some(key) = key {
                            let value = endpoint
                                .query
                                .get(&key)
                                .unwrap_or_else(|| panic!("No {} query param found", key.red()));

                            println!("{}", value);
                        } else {
                            println!("{}", endpoint.query_string());
                        }
                    }
                    EndpointShowCommands::Headers { key } => {
                        if let Some(key) = key {
                            let value = endpoint
                                .headers
                                .get(&key)
                                .unwrap_or_else(|| panic!("No {} header found", key.red()));

                            println!("{}", value);
                        } else {
                            println!("{}", endpoint.headers);
                        }
                    }
                    EndpointShowCommands::Body => {
                        if let Some(chunk) = endpoint.body(&handle).data().await {
                            stdout().write_all(&chunk.unwrap()).await.unwrap();
                        }
                    }
                }
            } else {
                let (_, endpoint) = ctx.require_endpoint();

                println!("{}", endpoint.to_toml().unwrap());
            }
        }
        Commands::Edit { editor } => {
            let specification = ctx.require_handle();

            if let Some(editor) = editor {
                ctx.config.preferences.editor = editor;
            }

            ctx.edit(
                &specification.dir().join("endpoint.toml"),
                validator::toml_as::<Endpoint>,
            )
            .unwrap();
        }
        Commands::Remove { handle } => {
            let specification = ctx.require_input_handle(&handle);

            if std::fs::remove_dir_all(specification.dir()).is_ok() {
                println!("Deleted endpoint {}", specification.handle());
            } else {
                panic!("failed to delete endpoint {}", specification.handle());
            }
        }
        Commands::Query { command } => {
            if let Some(command) = command {
                match command {
                    cli::EndpointQueryCommands::Get { key } => {
                        let (_, endpoint) = ctx.require_endpoint();

                        let value = endpoint
                            .query
                            .get(&key)
                            .unwrap_or_else(|| panic!("no query param {} found", key.red()));

                        println!("{value}");
                    }
                    cli::EndpointQueryCommands::Set { query: queries } => {
                        let (handle, mut endpoint) = ctx.require_endpoint();

                        for input in queries {
                            endpoint.query.set(&input);
                        }

                        endpoint.write(&handle);
                    }
                    cli::EndpointQueryCommands::Remove { key } => {
                        let (handle, mut endpoint) = ctx.require_endpoint();

                        endpoint.query.remove(&key);

                        endpoint.write(&handle);
                    }
                    cli::EndpointQueryCommands::List => {
                        let (_, endpoint) = ctx.require_endpoint();

                        println!("{}", endpoint.query);
                    }
                }
            } else {
                let (_, endpoint) = ctx.require_endpoint();
                println!("{}", endpoint.query_string());
            }
        }
        Commands::Header { command } => {
            if let Some(command) = command {
                match command {
                    cli::EndpointHeaderCommands::Get { key } => {
                        let (_, endpoint) = ctx.require_endpoint();
                        if let Some(header) = endpoint.headers.get(&key) {
                            println!("{}", header);
                        } else {
                            panic!("no header named {} found", key);
                        }
                    }
                    cli::EndpointHeaderCommands::Set { header: headers } => {
                        let (handle, mut endpoint) = ctx.require_endpoint();

                        for input in headers {
                            endpoint.headers.set(&input);
                        }

                        endpoint.write(&handle);
                    }
                    cli::EndpointHeaderCommands::Remove { key: keys } => {
                        let (handle, mut endpoint) = ctx.require_endpoint();

                        for k in keys {
                            endpoint.headers.remove(&k);
                        }

                        endpoint.write(&handle);
                    }
                    cli::EndpointHeaderCommands::List => {
                        let (_, endpoint) = ctx.require_endpoint();

                        println!("{}", endpoint.headers);
                    }
                }
            } else {
                let (_, endpoint) = ctx.require_endpoint();
                println!("{}", endpoint.headers);
            }
        }
        Commands::Body {
            stdin: expects_stdin,
            edit: should_edit,
            print: should_print,
        } => {
            let (handle, mut endpoint) = ctx.require_endpoint();

            let mut body = endpoint.body(&handle);

            if expects_stdin {
                let mut input = String::new();

                while let Ok(bytes) = std::io::stdin().read_line(&mut input) {
                    if bytes == 0 {
                        break;
                    }
                }

                body = Body::from(input);
            }

            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(handle.dir().join("body.json"))
            {
                while let Some(chunk) = body.data().await {
                    let _ = file.write_all(&chunk.unwrap());
                }
            }

            if should_edit {
                ctx.edit(&handle.dir(), validator::json).unwrap();
            }

            if should_print {
                if let Some(chunk) = endpoint.body(&handle).data().await {
                    stdout().write_all(&chunk.unwrap()).await.unwrap();
                }
            }

            endpoint.write(&handle);
        }
        Commands::Last {
            command: maybe_command,
            date: date_format,
        } => {
            let mut entry = History::last().expect("no history found");

            if let Some(format) = date_format {
                entry.date_format(format);
            }

            if maybe_command.is_none() {
                return println!("{entry}");
            }

            if let Some(command) = maybe_command {
                match command {
                    cli::LastCommands::Handle => {
                        println!("{}", entry.handle)
                    }
                    cli::LastCommands::Date => {
                        println!("{}", entry.date().unwrap_or("Unknown".into()));
                    }
                    cli::LastCommands::Request { command } => match command {
                        cli::LastRequestCommands::Url => println!("{}", entry.request.endpoint.url),
                        cli::LastRequestCommands::Query => {
                            println!("{}", entry.request.endpoint.query)
                        }
                        cli::LastRequestCommands::Headers => {
                            println!("{}", entry.request.endpoint.headers)
                        }
                        cli::LastRequestCommands::Method => {
                            println!("{}", entry.request.endpoint.method)
                        }
                        cli::LastRequestCommands::Body => println!("{}", entry.request.body),
                        cli::LastRequestCommands::Context => {
                            println!("{}", entry.request.context.name)
                        }
                    },
                    cli::LastCommands::Response { command } => match command {
                        cli::LastResponseCommands::Status => println!("{}", entry.response.status),
                        cli::LastResponseCommands::Headers => {
                            println!("{}", entry.response.headers)
                        }
                        cli::LastResponseCommands::Body => println!("{}", entry.response.body),
                        cli::LastResponseCommands::Size => println!("{}", entry.response.size),
                    },
                }
            }
        }
        Commands::History {
            max_count,
            date,
            show: show_fields,
        } => {
            let history = History::new().unwrap();
            let mut count = 0;
            let max_count = max_count.unwrap_or(usize::MAX);
            let format = date.unwrap_or(history::DEFAULT_DATE_FORMAT.into());

            for mut entry in history {
                entry.date_format(format.clone());

                if count >= max_count {
                    break;
                }

                count += 1;
                if count != 1 {
                    println!();
                }

                if show_fields.is_empty() {
                    println!("{entry}");
                    continue;
                }

                let mut outputs: Vec<String> = Vec::new();
                for key in &show_fields {
                    let value = entry
                        .field_as_string(key)
                        .unwrap_or_else(|_| panic!("invalid field: {}", key.red()));

                    outputs.push(value);
                }

                for value in outputs {
                    println!("{}", value);
                }
            }
        }
        Commands::Variable { command } => {
            let mut context = ctx.require_context();

            if let Some(command) = command {
                match command {
                    cli::VariableCommands::Get { key } => {
                        let v = context
                            .variables
                            .get(&key)
                            .unwrap_or_else(|| panic!("{} variable not set", key));

                        println!("{}", v);
                    }
                    cli::VariableCommands::Set {
                        variable: variables,
                    } => {
                        for input in variables {
                            context.variables.set(&input);
                        }

                        context.update().unwrap();
                    }
                    cli::VariableCommands::List => {
                        println!("{}", context.variables);
                    }
                    cli::VariableCommands::Edit => {
                        ctx.edit(&context.dir().join("variables.toml"), validator::toml)
                            .unwrap();
                    }
                    cli::VariableCommands::Remove { key } => {
                        context
                            .variables
                            .remove(&key)
                            .unwrap_or_else(|| panic!("{} variable not set", key));
                    }
                };
            } else {
                println!("{}", context.variables);
            }
        }
        Commands::Context { command } => match command {
            cli::ContextCommands::Create { name } => {
                let context = Context::new(&name);

                if context.exists() {
                    panic!("a context named {} already exists", name.red());
                }

                if context.write().is_err() {
                    panic!("failed to create {} context", name);
                }
            }
            cli::ContextCommands::Copy { src, dest } => {
                let src = Context::parse(&src).unwrap_or_else(|_| {
                    panic!("no {} context found", &src);
                });
                let mut dest = Context::parse(&dest).unwrap_or(Context::new(&dest));

                for (key, value) in src.variables.iter() {
                    dest.variables.insert(key.to_string(), value.to_string());
                }

                if dest.exists() {
                    dest.update().unwrap();
                } else {
                    dest.write().unwrap();
                }
            }
            cli::ContextCommands::Use { context } => {
                let context = Context::new(&context);

                if !context.exists() {
                    panic!("context {} does not exist", context.name.red());
                }

                if let Ok(()) = StateField::Context.set(&context.name) {
                    println!("Switched to {} context", context.name.green());
                } else {
                    panic!("failed to switch to {} context", context.name.red());
                }
            }
            cli::ContextCommands::List => {
                if let Ok(entries) = std::fs::read_dir(Path::new(".quartz").join("contexts")) {
                    for entry in entries {
                        let bytes = entry.unwrap().file_name();
                        let context_name = bytes.to_str().unwrap();

                        let state = ctx
                            .state
                            .get(StateField::Context)
                            .unwrap_or(String::from("default"));

                        if state == context_name {
                            println!("* {}", context_name.green());
                        } else {
                            println!("  {}", context_name);
                        }
                    }
                }
            }
            cli::ContextCommands::Remove { context } => {
                let context = Context::new(&context);

                if !context.exists() {
                    panic!("context {} does not exist", context.name.red());
                }

                if std::fs::remove_dir_all(context.dir()).is_ok() {
                    println!("Deleted {} context", context.name.green());
                } else {
                    panic!("failed to delete {} context", context.name.red());
                }
            }
        },
        Commands::Config { command } => match command {
            cli::ConfigCommands::Get { key } => {
                let value: String = match key.as_str() {
                    "preferences.editor" => ctx.config.preferences.editor,
                    "ui.colors" => ctx.config.ui.colors.to_string(),
                    _ => panic!("invalid key"),
                };

                println!("{value}");
            }
            cli::ConfigCommands::Edit => {
                ctx.edit(&Config::filepath(), validator::toml_as::<Config>)
                    .unwrap();
            }
            cli::ConfigCommands::Set { key, value } => {
                match key.as_str() {
                    "preferences.editor" => ctx.config.preferences.editor = value,
                    "ui.colors" => ctx.config.ui.colors = matches!(value.as_str(), "true"),
                    _ => panic!("invalid key"),
                };

                if ctx.config.write().is_err() {
                    panic!("failed to save config change");
                }
            }
            cli::ConfigCommands::List => {
                let content = toml::to_string(&ctx.config)
                    .unwrap_or_else(|_| panic!("could not parse configuration file"));

                println!("{content}");
            }
        },
    }
}
