This is a redbit workflow to get you up and running :

### Server

Copy Cargo.toml and mimic `src/main.rs` and `src/lib.rs` and start the server :
```bash
cd examples/utxo
cargo run
```
That will start the server http://127.0.0.1:8000/swagger-ui/ which is serving the http://127.0.0.1:8000/openapi.json

### Client 

Copy all the remaining client-side files and test that the typescript client generated from the OpenAPI spec works:
```bash
./bin/test.sh
```