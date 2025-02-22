Redbit reads struct annotations and derives code necessary for persisting and querying structured data into/from redb
using secondary indexes and dictionaries, let's say we want to persist Utxo into Redb using Redbit :

Declare annotated Struct `examples/utxo/src/lib.rs`:

<!-- BEGIN_LIB -->
<!-- END_LIB -->

And R/W entire instances efficiently using indexes and dictionaries `examples/utxo/src/main.rs`:  

<!-- BEGIN_MAIN -->
<!-- END_MAIN -->
