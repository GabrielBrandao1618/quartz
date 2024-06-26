use crate::{
    cookie::CookieJar,
    endpoint::EndpointPatch,
    history::{self, History},
    Ctx, PairMap, QuartzResult,
};
use chrono::Utc;
use hyper::{
    body::{Bytes, HttpBody},
    header::{HeaderName, HeaderValue},
    Body, Client, Uri,
};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tokio::io::{stdout, AsyncWriteExt as _};

#[derive(clap::Args, Debug)]
pub struct Args {
    /// Change a variable when sending the request.
    #[arg(long = "var", short = 'v', value_name = "KEY=VALUE")]
    variables: Vec<String>,

    #[command(flatten)]
    patch: EndpointPatch,

    /// Do not follow redirects
    #[arg(long)]
    no_follow: bool,

    /// Pass cookie data to request header
    #[arg(long = "cookie", short = 'b', value_name = "DATA|FILENAME")]
    cookies: Vec<String>,

    /// Which file to write all cookies after a completed request
    #[arg(long, short = 'c', value_name = "FILE")]
    cookie_jar: Option<PathBuf>,
}

pub async fn cmd(ctx: &Ctx, mut args: Args) -> QuartzResult {
    let (handle, mut endpoint) = ctx.require_endpoint();
    let mut env = ctx.require_env();
    for var in args.variables {
        env.variables.set(&var);
    }

    if !endpoint.headers.contains_key("user-agent") {
        endpoint
            .headers
            .insert("user-agent".to_string(), Ctx::user_agent());
    }

    let mut cookie_jar = env.cookie_jar(ctx);

    let extras = args.cookies.iter().flat_map(|c| {
        if c.contains('=') {
            return vec![c.to_owned()];
        }

        let path = Path::new(c);
        if !path.exists() {
            panic!("no such file: {c}");
        }

        CookieJar::read(path)
            .unwrap()
            .iter()
            .map(|c| format!("{}={}", c.name(), c.value()))
            .collect()
    });

    let cookie_value = cookie_jar
        .iter()
        .map(|c| format!("{}={}", c.name(), c.value()))
        .chain(extras)
        .collect::<Vec<String>>()
        .join("; ");

    if !cookie_value.is_empty() {
        endpoint
            .headers
            .insert(String::from("Cookie"), cookie_value);
    }

    let mut entry = history::Entry::builder();
    entry
        .handle(handle.handle())
        .timestemp(Utc::now().timestamp_micros());

    endpoint.update(&mut args.patch);
    endpoint.apply_env(&env);

    let body = endpoint.body().cloned();

    let mut res: hyper::Response<Body>;

    loop {
        let mut req = endpoint
            // TODO: Find a way around this clone
            .clone()
            .into_request()
            .unwrap_or_else(|_| panic!("malformed request"));
        for (key, val) in env.headers.iter() {
            if !endpoint.headers.contains_key(key) {
                req.headers_mut()
                    .insert(HeaderName::from_str(key)?, HeaderValue::from_str(val)?);
            }
        }

        entry.message(&req);
        if let Some(ref body) = body {
            entry.message_raw(body.to_owned());
        }

        let client = {
            let https = hyper_tls::HttpsConnector::new();
            Client::builder().build(https)
        };

        res = client.request(req).await?;

        entry.message(&res);

        if let Some(cookie_header) = res.headers().get("Set-Cookie") {
            let url = endpoint.full_url()?;

            cookie_jar.set(url.host().unwrap(), cookie_header.to_str()?);
        }

        if args.no_follow || !res.status().is_redirection() {
            break;
        }

        if let Some(location) = res.headers().get("Location") {
            let location = location.to_str()?;

            if location.starts_with('/') {
                let url = endpoint.full_url()?;
                // This is awful
                endpoint.url = Uri::builder()
                    .authority(url.authority().unwrap().as_str())
                    .scheme(url.scheme().unwrap().as_str())
                    .path_and_query(location)
                    .build()?
                    .to_string();
            } else if Uri::from_str(location).is_ok() {
                endpoint.url = location.to_string();
            }
        };
    }

    match args.cookie_jar {
        Some(path) => cookie_jar.write_at(&path)?,
        None => cookie_jar.write()?,
    };

    let mut bytes = Bytes::new();

    while let Some(chunk) = res.data().await {
        if let Ok(chunk) = chunk {
            bytes = [bytes, chunk].concat().into();
        }
    }

    entry.message_raw(String::from_utf8(bytes.to_vec())?);

    let _ = stdout().write_all(&bytes).await;
    History::write(ctx, entry.build()?)?;

    Ok(())
}
