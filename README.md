Redbit reads struct annotations and derives code necessary for persisting and querying structured data into/from redb
using secondary indexes and dictionaries, let's say we want to persist Utxo into Redb using Redbit :

### Main motivations
- ✅ Achieving more advanced querying capabilities with embedded KV stores is non-trivial  
- ✅ Absence of any existing abstraction layer for structured data  
- ✅ Handwriting custom codecs on byte-level is tedious and painful

### Major Out-of-the-Box Features

- ✅ Querying and ranging by secondary index
- ✅ Optional dictionaries for low cardinality fields
- ✅ One-to-One and One-to-Many entities with cascade read/write/delete
- ✅ All goodies including intuitive data ordering without writing custom codecs

Declare annotated Struct `examples/utxo/src/lib.rs`:

<!-- BEGIN_LIB -->
<!-- END_LIB -->

And R/W entire instances efficiently using indexes and dictionaries `examples/utxo/src/main.rs`:  

<!-- BEGIN_MAIN -->
<!-- END_MAIN -->

Performance wise, check 🔥[flamegraph](https://rawcdn.githack.com/pragmaxim-com/redbit/refs/heads/master/flamegraph.svg).
Instances are persisted completely structured by fields which means Redbit has slower write performance but blazing fast reads.

