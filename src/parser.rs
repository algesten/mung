#![allow(clippy::needless_lifetimes)]

use crate::token::{TokenKind, Tokens};
use serde::Deserialize;
use std::fmt;
use std::io;

#[derive(Debug)]
pub struct Expr {
    pub collection: String,
    pub oper: Oper,
}

#[derive(Debug)]
pub enum Oper {
    Find {
        doc: Option<String>,
        proj: Option<String>,
        cursor: CursorOpts,
    },
    Count {
        doc: Option<String>,
    },
    Distinct {
        field: String,
        doc: Option<String>,
    },
    Update {
        query: String,
        upd: String,
        uopts: UpdateOpts,
    },
    Insert {
        doc: String,
    },
    Remove {
        doc: String,
    },
}

#[derive(Debug, Default)]
pub struct CursorOpts {
    pub batch_size: Option<u32>,
    pub limit: Option<i64>,
    pub skip: Option<i64>,
    pub sort: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UpdateOpts {
    pub multi: Option<bool>,
    pub upsert: Option<bool>,
}

pub fn parse<B: io::BufRead>(tok: &mut Tokens<B>) -> Result<Option<Expr>, String> {
    debug!("Parse expression");

    tok.skip_white();

    // end of stream
    if tok.peek().is_none() {
        debug!("End of tokens");
        return Ok(None);
    }

    trace!("Parse db");

    let db = tok.expect_name()?;
    if db != "db" {
        return Err("Expected 'db'".into());
    }

    trace!("parse collection");

    tok.expect_kind(TokenKind::FullStop)?;
    let collection = tok.expect_name()?;

    tok.expect_kind(TokenKind::FullStop)?;

    let oper = parse_oper(tok)?;

    Ok(Some(Expr { collection, oper }))
}

fn parse_oper<B: io::BufRead>(mut tok: &mut Tokens<B>) -> Result<Oper, String> {
    trace!("parse_oper");
    let name = tok.expect_name()?;
    let par_tok = tok.find_pair(TokenKind::ParenLeft, TokenKind::ParenRight, false, false)?;

    match &name[..] {
        "find" => {
            let mut oper = parse_find(par_tok)?;
            // parse cursor options
            while tok.peek_kind() == Some(TokenKind::FullStop) {
                tok.expect_kind(TokenKind::FullStop)?;
                if let Oper::Find { cursor, .. } = &mut oper {
                    parse_cursor_opt(&mut tok, cursor)?;
                }
            }
            Ok(oper)
        }
        "count" => parse_count(par_tok),
        "distinct" => parse_distinct(par_tok),
        "update" => parse_update(par_tok),
        "insert" => parse_insert(par_tok),
        "remove" => parse_remove(par_tok),
        _ => Err(format!("Unhandled operation: {}", name)),
    }
}

fn parse_find<B: io::BufRead>(mut tok: Tokens<B>) -> Result<Oper, String> {
    trace!("parse_find");
    let doc = maybe_expect_doc(&mut tok)?;
    let proj = if doc.is_some() && tok.peek_kind() == Some(TokenKind::Comma) {
        tok.expect_kind(TokenKind::Comma)?;
        maybe_expect_doc(&mut tok)?
    } else {
        None
    };

    let cursor = CursorOpts {
        ..Default::default()
    };

    Ok(Oper::Find { doc, proj, cursor })
}

fn parse_count<B: io::BufRead>(mut tok: Tokens<B>) -> Result<Oper, String> {
    trace!("parse_count");
    let doc = maybe_expect_doc(&mut tok)?;
    Ok(Oper::Count { doc })
}

fn parse_distinct<B: io::BufRead>(mut tok: Tokens<B>) -> Result<Oper, String> {
    trace!("parse_distinct");
    tok.skip_white();
    let field = tok.expect_string(false)?;
    tok.skip_white();
    if tok.peek_kind() == Some(TokenKind::Comma) {
        tok.expect_kind(TokenKind::Comma)?;
    }
    let doc = maybe_expect_doc(&mut tok)?;
    Ok(Oper::Distinct { field, doc })
}

fn parse_update<B: io::BufRead>(mut tok: Tokens<B>) -> Result<Oper, String> {
    trace!("parse_update");
    let query = maybe_expect_doc(&mut tok)?.ok_or("Update requires a query")?;
    tok.expect_kind(TokenKind::Comma)?;
    let upd = maybe_expect_doc(&mut tok)?.ok_or("Update requires an update")?;

    let mut opts: Option<UpdateOpts> = None;

    if tok.peek_kind() == Some(TokenKind::Comma) {
        tok.expect_kind(TokenKind::Comma)?;
        let opts_doc = maybe_expect_doc(&mut tok)?;
        if let Some(opts_doc) = opts_doc {
            opts = Some(json5::from_str(&opts_doc).map_err(|e| e.to_string())?);
        }
    }

    let uopts = opts.unwrap_or(UpdateOpts {
        ..Default::default()
    });

    Ok(Oper::Update { query, upd, uopts })
}

fn parse_insert<B: io::BufRead>(mut tok: Tokens<B>) -> Result<Oper, String> {
    trace!("parse_insert");
    let mut doc = maybe_arr(&mut tok)?;
    if doc.is_none() {
        doc = maybe_expect_doc(&mut tok)?;
    }
    let doc = doc.ok_or("Insert needs a document")?;
    Ok(Oper::Insert { doc })
}

fn parse_remove<B: io::BufRead>(mut tok: Tokens<B>) -> Result<Oper, String> {
    trace!("parse_remove");
    let doc = maybe_expect_doc(&mut tok)?.ok_or("Remove needs a document")?;
    Ok(Oper::Remove { doc })
}

fn parse_cursor_opt<B: io::BufRead>(
    tok: &mut Tokens<B>,
    opts: &mut CursorOpts,
) -> Result<(), String> {
    trace!("parse_cursor_opt");
    let name = tok.expect_name()?;
    let mut par_tok = tok.find_pair(TokenKind::ParenLeft, TokenKind::ParenRight, false, false)?;
    par_tok.skip_white();
    match &name[..] {
        "batchSize" => {
            opts.batch_size = Some(par_tok.expect_as()?);
        }
        "limit" => {
            opts.limit = Some(par_tok.expect_as()?);
        }
        "skip" => {
            opts.skip = Some(par_tok.expect_as()?);
        }
        "sort" => {
            opts.sort = maybe_expect_doc(&mut par_tok)?;
            if opts.sort.is_none() {
                return Err("Expected doc for sort()".into());
            }
        }
        _ => return Err(format!("Unrecognized cursor option: {}", name)),
    }
    Ok(())
}

fn maybe_expect_doc<B: io::BufRead>(tok: &mut Tokens<B>) -> Result<Option<String>, String> {
    tok.skip_white();
    if tok.peek_kind().is_some() {
        let c_tok = tok.find_pair(TokenKind::CurlLeft, TokenKind::CurlRight, true, false)?;
        let doc = Some(c_tok.into_string());
        tok.skip_white();
        Ok(doc)
    } else {
        Ok(None)
    }
}

fn maybe_arr<B: io::BufRead>(tok: &mut Tokens<B>) -> Result<Option<String>, String> {
    tok.skip_white();
    if tok.peek_kind() == Some(TokenKind::BracketLeft) {
        let c_tok = tok.find_pair(TokenKind::BracketLeft, TokenKind::BracketRight, true, false)?;
        let doc = Some(c_tok.into_string());
        tok.skip_white();
        Ok(doc)
    } else {
        Ok(None)
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
