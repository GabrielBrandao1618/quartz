use core::panic;
use std::process::ExitCode;

use crate::{
    cli::{EnvCmd as Cmd, HeaderCmd},
    Ctx, Env, PairMap, QuartzResult, StateField,
};
use colored::Colorize;

#[derive(clap::Args, Debug)]
pub struct CreateArgs {
    name: String,
}

#[derive(clap::Args, Debug)]
pub struct CpArgs {
    src: String,
    dest: String,
}

#[derive(clap::Args, Debug)]
pub struct SwitchArgs {
    env: String,
}

#[derive(clap::Args, Debug)]
pub struct RmArgs {
    env: String,
}

pub fn cmd(ctx: &mut Ctx, command: Cmd) -> QuartzResult {
    match command {
        Cmd::Create(args) => create(ctx, args),
        Cmd::Cp(args) => cp(ctx, args)?,
        Cmd::Use(args) => switch(ctx, args)?,
        Cmd::Ls => ls(ctx),
        Cmd::Rm(args) => rm(ctx, args),
        Cmd::Header { command } => match command {
            HeaderCmd::Set { header } => header_set(ctx, header)?,
            HeaderCmd::Ls => header_ls(ctx)?,
            HeaderCmd::Rm { key } => header_rm(ctx, key)?,
            HeaderCmd::Get { key } => header_get(ctx, key)?,
        },
    };

    Ok(())
}

pub fn create(ctx: &Ctx, args: CreateArgs) {
    let env = Env::new(&args.name);

    if env.exists(ctx) {
        panic!("a environment named {} already exists", args.name.red());
    }

    if env.write(ctx).is_err() {
        panic!("failed to create {} environment", args.name);
    }
}

pub fn cp(ctx: &Ctx, args: CpArgs) -> QuartzResult {
    let src = Env::parse(ctx, &args.src).unwrap_or_else(|_| {
        panic!("no {} environment found", &args.src);
    });
    let mut dest = Env::parse(ctx, &args.dest).unwrap_or(Env::new(&args.dest));

    for (key, value) in src.variables.iter() {
        dest.variables.insert(key.to_string(), value.to_string());
    }

    if dest.exists(ctx) {
        dest.update(ctx)?;
    } else {
        dest.write(ctx)?;
    }

    Ok(())
}

pub fn switch(ctx: &mut Ctx, args: SwitchArgs) -> QuartzResult {
    let env = Env::new(&args.env);

    if !env.exists(ctx) {
        println!("Environment {} doesn't exist", env.name.red());
        if ctx.confirm("Do you wish to create it?") {
            create(
                ctx,
                CreateArgs {
                    name: env.name.clone(),
                },
            );
        } else {
            ctx.code(ExitCode::FAILURE);
            return Ok(());
        }
    }

    if let Ok(()) = StateField::Env.set(ctx, &env.name) {
        println!("Switched to {} environment", env.name.green());
    } else {
        panic!("failed to switch to {} environment", env.name.red());
    }

    Ok(())
}

pub fn ls(ctx: &Ctx) {
    if let Ok(entries) = std::fs::read_dir(ctx.path().join("env")) {
        for entry in entries {
            let bytes = entry.unwrap().file_name();
            let env_name = bytes.to_str().unwrap();

            let state = ctx
                .state
                .get(ctx, StateField::Env)
                .unwrap_or(String::from("default"));

            if state == env_name {
                println!("* {}", env_name.green());
            } else {
                println!("  {}", env_name);
            }
        }
    }
}

pub fn rm(ctx: &Ctx, args: RmArgs) {
    let env = Env::new(&args.env);

    if !env.exists(ctx) {
        panic!("environment {} does not exist", env.name.red());
    }

    if std::fs::remove_dir_all(env.dir(ctx)).is_ok() {
        println!("Deleted {} environment", env.name.green());
    } else {
        panic!("failed to delete {} environment", env.name.red());
    }
}

pub fn print(ctx: &Ctx) {
    println!(
        "{}",
        ctx.state
            .get(ctx, StateField::Env)
            .unwrap_or("default".into())
    );
}
pub fn header_set(ctx: &Ctx, args: Vec<String>) -> QuartzResult {
    let mut env = ctx.require_env();
    for header in args {
        env.headers.set(&header);
    }
    env.update(ctx)?;
    Ok(())
}
pub fn header_ls(ctx: &Ctx) -> QuartzResult {
    let env = ctx.require_env();
    print!("{}", env.headers);
    Ok(())
}
pub fn header_rm(ctx: &Ctx, keys: Vec<String>) -> QuartzResult {
    for key in keys {
        let mut env = ctx.require_env();
        env.headers.remove(&key);
        env.update(ctx)?;
    }
    Ok(())
}
pub fn header_get(ctx: &Ctx, key: String) -> QuartzResult {
    let env = ctx.require_env();
    let value = env
        .headers
        .get(&key)
        .unwrap_or_else(|| panic!("no header named {key} found"));
    println!("{value}");
    Ok(())
}
