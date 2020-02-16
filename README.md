mung
====

> mongodb tool with less suck.

[MongoDB](https://www.mongodb.com) is an acquired taste. One of it's
bigger shortcomings is the lack of reasonable tooling for handling the
data. The mongo shell doesn't make a distinction between stdout/stderr
and is very difficult to use as part of CLI scripts, piping, etc.

[`jq`](https://stedolan.github.io/jq/) has become a defacto standard
for wrangling JSON from the command line. The intention is that `mung`
works well in tandem with `jq` to query and manipulate data in
MongoDB.

## Scope

`mung` focuses on a narrow subset of everything that can be done in
mongo shell. The aim is querying and data manipulation. Topics such as
managing databases, index and clusters are (for now), out of scope.

`mung` is opinionated about keeping the syntax simple. MongoDB full
syntax is extremely complex and often have overlapping ways of
achieving similar things.

Mung is written in Rust and relies on the [official MongoDB Rust
driver](https://crates.io/crates/mongodb) which is currently in alpha
(BE WARNED!).

## Install

You need rust 1.39+ installed. https://rustup.rs

```bash
$ cargo install --git ssh://git@github.com/algesten/mung
```

## Help

`mung -h` shows usage help.

```
mung 0.1.0
mongodb tool with less suck

USAGE:
    mung [FLAGS] [OPTIONS] <COMMAND>

FLAGS:
    -c, --compact     Compact instead of pretty printed output
    -h, --help        Prints help information
    -W, --password    Prompt for password
    -V, --version     Prints version information
    -v, --verbose     Verbose mode (-v, -vv, -vvv, etc.)

OPTIONS:
    -d, --dbname <dbname>    Database to use [env: MONGO_DB=]  [default: test]
    -u, --url <url>          URL to connect to [env: MONGO_URL]  [default: mongodb://127.0.0.1:27017]

ARGS:
    <COMMAND>    Command to run or "-" to read from stdin
```

## Connect to a DB

`mung` uses the URL form for connecting to MongoDB. The argument is either passed
on the command line using `-u`, or read from the environment variable `MONGO_URL`.

The following forms are accepted:

  * `mongodb://user:pass@myhost`
  * `mongodb+srv://user:pass@mycluster`

The password can be read from stdin by using the `-W` option.

The rust mongodb driver accepts many [query
parameters](https://docs.rs/mongodb/0.9.1/mongodb/options/struct.ClientOptions.html#method.parse)
to modify the connection behavior

```bash
$ export MONGO_URL="mongodb+srv://dbUser:dbUserPassword@clusterx-abc123.mongodb.net"
$ mung -d production 'db.users.find({ username: "martin" })'
```

```bash
$ mung -u "mongodb+srv://dbUser:dbUserPassword@clusterx-abc123.mongodb.net" \
       -d production 'db.users.find({ username: "martin" })'
```

### Select database

The default database is `test`, and is changed using the `-d`
parameter. `mung` differs from mongo shell in that it ignores any
database passed in the URL. The command line argument is _always_ used
(or defaulting to `test` if not present).

## Commands

The commands tries to be as close to mongo shell as possible.

All commands have the form:

`db.<collection>.<command>([doc], ...)`

They all start with `db.` (which is the database pointed out by the
`-d` parameter. Db is followed by the `collection` name to make
operations on and then the `command` to run.

Commands are either read from the command line, or from stdin using
`-`. These are equivalent:

  * `mung -d prod 'db.user.find()'`
  * `echo 'db.user.find()' | mung -d prod -`

### Streaming

Multiple commands are separated by whitespace, parsed and executed one
by one in a streaming fashion. That means we can construct pipes that
are not doing any unecessary buffering for combined operations like:

```bash
$ mung -d test "db.users.find().limit(3)" \
    | jq -r ._id \
    | xargs -I % echo 'db.users.remove({ _id: "%" })' \
    | mung -d test -
```

This is an example to illustrate a feature and not the best way to do
this. Let's break that down.

  1. `mung -d test "db.users.find().limit(3)"` find three users and
     output the entire json to stdout.
  2. `jq -r ._id` of the json, just pick the `_id` field.
  3. `xargs -I % echo 'db.users.remove({ _id: "%" })'` Row-by-row
     construct a new command to stdout.
  4. `mung -d test -`. One by one, read the commands from stdin and 
     execute them.

### Shell escaping

Mongo's query language makes extensive use of `$` Depending on shell,
this might clash with variable substituion syntax. For bash this works:

  * `mung -d prod 'db.user.find({ age: { $gt: 42 } })'` (single quote)
  * `mung -d prod "db.user.find({ age: { \$gt: 42 } })"` (double quote
    and `\$`)

## `db.collection.find(<query>, <projection>)`

Queries for documents. See [mongo
doc](https://docs.mongodb.com/manual/reference/method/db.collection.find/)
for how to construct queries.

Both `query` and `projection` are optional. Without any arguments, all
documents are returned.

### JSONL not Array

The output from `find()` is streaming, which means each JSON doc is
printed straight to stdout. When `find()` returns multiple documents,
each doc is printed after another without being wrapped in an
array or separated by commas.

```json
{"doc": 1}
{"doc": 2}
```

The default is not strict JSONL, because all output is pretty printed
which means newline does not mean a new value. By using the `-c`
(compact) option, we get JSONL. It is the same behavior as `jq`.

```bash
$ mung -d prod -c 'db.users.find({}, {_id: 1})'
{"_id": "user1"}
{"_id": "user2"}
...
```

If you want an array, use `jq`'s "slurp" feature:

```bash
$ mung -d prod 'db.users.find({}, {_id: 1})' | jq '._id' | jq -s
[
  "user1",
  "user2",
  ...
]
```

Examples

  * `mung -d prod 'db.users.find()'`
  * `mung -d prod 'db.users.find({ age: { $gt: 42 } })'`
  * `mung -d prod 'db.users.find({ age: { $gt: 42 } }, { name: 1 })'`

### Sorting

Sorting is added as a tail to the find command and works like in
[mongo
shell](https://docs.mongodb.com/manual/reference/method/cursor.sort/).

`db.collection.find(...).sort(<sort>)`

Example

  * `mung -d prod 'db.users.find().sort({ age: -1 }'`

### `limit`, `skip` and `batchSize`

These options are added to the tail and works like in mongo shell.

 * `db.collection.find().limit(3)` (return 3 results). [See mongo
   doc](https://docs.mongodb.com/manual/reference/method/cursor.limit/)
 * `db.collection.find().skip(3)` (skip first 3 results). [See mongo
   doc](https://docs.mongodb.com/manual/reference/method/cursor.skip/)
 * `db.collection.find().batchSize(1000)` (load 1000 results at a time). [See mongo
   doc](https://docs.mongodb.com/manual/reference/method/cursor.batchSize/)

## `db.collection.count(<query>)`

Counts number of matching documents like [mongo
shell](https://docs.mongodb.com/manual/reference/method/db.collection.count/).

Examples

  * `mung -d prod 'db.users.count()'`
  * `mung -d prod 'db.users.count({ age: { $gt: 42 } })'`

## `db.collection.distinct([field], <query>)`

Counts number of distinctly different values of `field` in
`collection. Optionally provides a `query` filter. [See mongo
docs](https://docs.mongodb.com/manual/reference/method/db.collection.distinct/).

Examples

  * `mung -d prod 'db.users.distinct('age')'` (How many different age
    values users)
  * `mung -d prod 'db.users.distinct('age', { age: { $gt: 42 } })'`
    (How many different age values of users over 42)

## `db.collection.insert([doc or array])`

Inserts one or many docs into collection. See [mongo
doc](https://docs.mongodb.com/manual/reference/method/db.collection.insert/).

Examples

  * `mung -d prod 'db.users.insert({ name: "martin", age: 34 })'`
  * `mung -d prod 'db.users.insert([ { name: "martin", age: 34 }, { name: "G", age: 34 } ])'`

## `db.collection.update([query], [update], <opts>)`

Updates one (or many with `opts.multi`) document. The `query` document
is what to match, and `update` is the update to run. See [mongo
doc](https://docs.mongodb.com/manual/reference/method/db.collection.update/)
for details.

By default, even if the query matches many documents, only one single
document is updated unless we pass `opts.multi`.

### Options

  * `multi` to update more than one doc.
  * `upsert` to fall back to an insert if the query didn't match
    anything. See [mongo
    doc](https://docs.mongodb.com/manual/reference/method/db.collection.update/#update-upsert)

Examples:

  * `mung -d prod 'db.users.update({ _id: 'abcdef123' }, { $set: {
    age: 43 } })'`. Update user with specific id and set `age` field
    to `43`.
  * `mung -d prod 'db.users.update({ age: { $gt: 42 } }, { $set: {
    cool: true } }, { multi: true })'`. Update all (multi) users over
    42 and set a field `cool` to `true`.

## `db.collection.remove([query])`

Removes one or many documents matching the query. Pass `{}` to remove
everything in the collection.

Examples:

  * `mung -d prod 'db.users.remove({ _id: 'abc123' })'`. Remove one
    document with specific id.
  * `mung -d prod 'db.users.remove({ name: "martin" })'`. Remove all
    users named martin.
  * `mung -d prod 'db.users.remove({})'`. Remove all users.

## Logging

Use `-v` to get more logging and `-vv` for max logging. Credentials
part of the URL will leak with logging turned on.

The `-v` targets only `mung` itself. To turn on logging for all
dependent libraries, use the `MUNG_LOG` environment variable
set to something like `MUNG_LOG=trace`.

## License

Copyright (c) 2020 Martin Algesten

- MIT license
  ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
