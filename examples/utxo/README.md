This is a redbit workflow to get you up and running :

### Server

Copy Cargo.toml and mimic `src/main.rs` and `src/lib.rs` and start the server :
```bash
cargo run
```
That will start the server http://127.0.0.1:8000/swagger-ui/ which is serving the http://127.0.0.1:8000/openapi.json

### Client 

Copy ui and run the build script :
```bash
./bin/build.sh
```