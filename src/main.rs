#![warn(clippy::all)]

#[macro_use]
extern crate log;

use structopt::StructOpt;

mod chars;
mod error;
mod parser;
mod token;

use crate::error::Error;
use crate::parser::CursorOpts;
use crate::parser::Oper;
use crate::parser::UpdateOpts;
use bson::Bson;
use colored_json::{ColorMode, ColoredFormatter, Output};
use mongodb::options::FindOptions;
use mongodb::options::UpdateModifications;
use mongodb::options::UpdateOptions;
use mongodb::sync::Collection;
use serde::Serialize;
use serde_json::ser::CompactFormatter;
use serde_json::ser::PrettyFormatter;
use serde_json::Value;
use std::sync::mpsc::sync_channel;
use std::sync::mpsc::Receiver;

/// mongodb tool with less suck.
#[derive(StructOpt, Debug)]
#[structopt(name = "mung")]
struct Opts {
    // The number of occurrences of the `v/verbose` flag
    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[structopt(short, long, parse(from_occurrences))]
    verbose: u8,

    /// Database to use
    #[structopt(short, long, env = "MONGO_DB", default_value = "test")]
    dbname: String,

    /// Compact instead of pretty printed output
    #[structopt(short, long)]
    compact: bool,

    /// URL to connect to
    #[structopt(
        short,
        long,
        env = "MONGO_URL",
        hide_env_values = true,
        default_value = "mongodb://127.0.0.1:27017"
    )]
    url: String,

    /// Command to run or "-" to read from stdin
    #[structopt(name = "COMMAND")]
    command: String,
}

const LOG_ENV_VAR: &str = "MUNG_LOG";

fn main() {
    let opts = Opts::from_args();

    if std::env::var(LOG_ENV_VAR).ok().is_none() {
        let level = match opts.verbose {
            0 => "mung=info",
            1 => "mung=debug",
            _ => "mung=trace",
        };
        std::env::set_var(LOG_ENV_VAR, level);
    }
    pretty_env_logger::init_custom_env(LOG_ENV_VAR);

    match handle(&opts) {
        Ok(_) => {
            debug!("Success");
            std::process::exit(0)
        }
        Err(e) => {
            error!("{}", e);
            std::process::exit(1)
        }
    }
}

use std::io;

fn handle(opts: &Opts) -> Result<(), Error> {
    //
    let read_stdin = opts.command.trim() == "-";

    debug!("Connect to db");
    let client = mongodb::sync::Client::with_uri_str(&opts.url)?;

    trace!("Use db: {}", opts.dbname);
    let mut db = client.database(&opts.dbname);

    if read_stdin {
        debug!("Read commands from stdin");
        let stdin = io::stdin();
        let lock = stdin.lock();
        let reader = io::BufReader::new(lock);
        let mut tokens = token::tokenize(reader);
        while let Some(expr) = parser::parse(&mut tokens)? {
            execute(&mut db, expr, &opts)?;
        }
    } else {
        debug!("Read commands from argument");
        let mut tokens = token::tokenize_str(&opts.command);
        while let Some(expr) = parser::parse(&mut tokens)? {
            execute(&mut db, expr, &opts)?;
        }
    };

    Ok(())
}

fn execute(db: &mut mongodb::sync::Database, expr: parser::Expr, opts: &Opts) -> Result<(), Error> {
    trace!("Use collection: {}", expr.collection);
    let coll = db.collection(&expr.collection);

    match expr.oper {
        Oper::Find { doc, proj, cursor } => handle_find(
            coll,
            doc.as_ref().map(|s| &s[..]),
            proj.as_ref().map(|s| &s[..]),
            cursor,
            &opts,
        )?,
        Oper::Count { doc } => handle_count(coll, doc.as_ref().map(|s| &s[..]), opts)?,
        Oper::Distinct { field, doc } => {
            handle_distinct(coll, &field, doc.as_ref().map(|s| &s[..]), opts)?
        }
        Oper::Update { query, upd, uopts } => handle_update(coll, &query, &upd, uopts, opts)?,
        Oper::Insert { doc } => handle_insert(coll, &doc, opts)?,
        Oper::Remove { doc } => handle_remove(coll, &doc, opts)?,
    }
    Ok(())
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct UpdateResult {
    nMatched: i64,
    nModified: i64,
    nUpserted: i64,
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct InsertResult {
    nInserted: usize,
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct RemoveResult {
    nRemoved: i64,
}

fn handle_find(
    coll: Collection,
    doc: Option<&str>,
    proj: Option<&str>,
    cursor: CursorOpts,
    opts: &Opts,
) -> Result<(), Error> {
    trace!("Decode doc to bson");
    let doc = decode_bson(doc.unwrap_or("{}"))?;
    trace!("Decode projection to bson");
    let proj = decode_bson(proj.unwrap_or("{}"))?;

    let mut find_opts = FindOptions::builder()
        .projection(Some(proj))
        .batch_size(cursor.batch_size)
        .limit(cursor.limit)
        .skip(cursor.skip)
        .build();

    if let Some(s) = cursor.sort {
        find_opts.sort = Some(decode_bson(&s)?);
    }

    debug!("Call find");
    let cursor = coll.find(doc, find_opts)?;
    write_cursor(cursor, opts)?;

    Ok(())
}

fn handle_count(coll: Collection, doc: Option<&str>, opts: &Opts) -> Result<(), Error> {
    trace!("Decode doc to bson");
    let doc = decode_bson(doc.unwrap_or("{}"))?;

    debug!("Call count_documents");
    let count = coll.count_documents(doc, None)?;
    let val = Value::Number(count.into());
    write(opts.compact, &val)?;
    println!();

    Ok(())
}

fn handle_distinct(
    coll: Collection,
    field: &str,
    doc: Option<&str>,
    opts: &Opts,
) -> Result<(), Error> {
    trace!("Decode doc to bson");
    let doc = decode_bson(doc.unwrap_or("{}"))?;

    debug!("Call distinct");
    let doc = coll.distinct(field, doc, None)?;

    let val = serde_json::to_value(&doc)?;
    write(opts.compact, &val)?;
    println!();

    Ok(())
}

fn handle_update(
    coll: Collection,
    query: &str,
    update: &str,
    uopts: UpdateOpts,
    opts: &Opts,
) -> Result<(), Error> {
    trace!("Decode query to bson");
    let query = decode_bson(query)?;

    trace!("Decode update to bson");
    let update = decode_bson(update)?;

    let update_mod = UpdateModifications::Document(update);

    let up_opts = UpdateOptions::builder().upsert(uopts.upsert).build();

    let res = if uopts.multi.unwrap_or(false) {
        debug!("Call update_many");
        coll.update_many(query, update_mod, up_opts)?
    } else {
        debug!("Call update_one");
        coll.update_one(query, update_mod, up_opts)?
    };

    let ures = UpdateResult {
        nMatched: res.matched_count,
        nModified: res.modified_count,
        nUpserted: res.upserted_id.map(|_| 1).unwrap_or(0),
    };

    let val = serde_json::to_value(&ures)?;
    write(opts.compact, &val)?;
    println!();

    Ok(())
}

fn handle_insert(coll: Collection, doc: &str, opts: &Opts) -> Result<(), Error> {
    // figure out if we're getting an array or doc
    let json: Value = json5::from_str(doc)?;
    if let Value::Array(arr) = json {
        debug!("Decode doc as array");

        let mut todo = vec![];
        for json in arr {
            let bson: Bson = bson::ser::to_bson(&json)?;
            if let Bson::Document(doc) = bson {
                todo.push(doc);
            } else {
                return Err(Error::Usage("Bson is not a Document".into()));
            };
        }

        debug!("Call insert_many");
        let res = coll.insert_many(todo, None)?;
        let ires = InsertResult {
            nInserted: res.inserted_ids.len(),
        };

        let val = serde_json::to_value(&ires)?;
        write(opts.compact, &val)?;
        println!();
    } else if json.is_object() {
        debug!("Decode doc as object");

        let bson: Bson = bson::ser::to_bson(&json)?;
        let doc = if let Bson::Document(doc) = bson {
            doc
        } else {
            return Err(Error::Usage("Bson is not a Document".into()));
        };

        debug!("Call insert_one");

        coll.insert_one(doc, None)?;
        let ires = InsertResult { nInserted: 1 };

        let val = serde_json::to_value(&ires)?;
        write(opts.compact, &val)?;
        println!();
    } else {
        return Err(Error::Usage("Insert requires an array or document".into()));
    };

    Ok(())
}

fn handle_remove(coll: Collection, doc: &str, opts: &Opts) -> Result<(), Error> {
    trace!("Decode doc to bson");
    let doc = decode_bson(doc)?;

    debug!("Call delete_many");
    let res = coll.delete_many(doc, None)?;
    let rres = RemoveResult {
        nRemoved: res.deleted_count,
    };

    let val = serde_json::to_value(&rres)?;
    write(opts.compact, &val)?;
    println!();

    Ok(())
}

fn decode_bson(s: &str) -> Result<bson::Document, Error> {
    let json: Value = json5::from_str(s)?;
    let bson: Bson = bson::ser::to_bson(&json)?;
    let doc = if let Bson::Document(doc) = bson {
        doc
    } else {
        return Err(Error::Usage("Bson is not a Document".into()));
    };
    Ok(doc)
}

fn write_cursor(cursor: mongodb::sync::Cursor, opts: &Opts) -> Result<(), Error> {
    debug!("Write result from cursor");
    let rx = read_cursor(cursor);
    for doc in rx.into_iter() {
        let doc = doc?;
        let val = serde_json::to_value(&doc)?;
        write(opts.compact, &val)?;
        println!();
    }
    Ok(())
}

fn read_cursor(cursor: mongodb::sync::Cursor) -> Receiver<Result<bson::Document, Error>> {
    let (tx, rx) = sync_channel(10_000);

    std::thread::spawn(move || {
        let mut alive = true;
        for doc in cursor {
            if !alive {
                break;
            }
            if doc.is_err() {
                alive = false;
            }
            if tx.send(doc.map_err(Error::MongoDb)).ok().is_none() {
                alive = false;
            }
        }
    });

    rx
}

#[allow(clippy::collapsible_if)]
fn write(compact: bool, value: &Value) -> Result<(), Error> {
    let color = ColorMode::Auto(Output::StdOut);
    let writer = std::io::stdout();

    if color.use_color() {
        if compact {
            let formatter = ColoredFormatter::new(CompactFormatter);
            let mut ser = serde_json::Serializer::with_formatter(writer, formatter);
            value.serialize(&mut ser)?;
        } else {
            let formatter = ColoredFormatter::new(PrettyFormatter::new());
            let mut ser = serde_json::Serializer::with_formatter(writer, formatter);
            value.serialize(&mut ser)?;
        }
    } else {
        if compact {
            let formatter = CompactFormatter;
            let mut ser = serde_json::Serializer::with_formatter(writer, formatter);
            value.serialize(&mut ser)?;
        } else {
            let formatter = PrettyFormatter::new();
            let mut ser = serde_json::Serializer::with_formatter(writer, formatter);
            value.serialize(&mut ser)?;
        }
    }

    Ok(())
}
