It syncs with a demo chain + showcase of the available API.

## Usage 

``` 
cargo run --release 
```

### Bench

```
cargo bench --bench demo_benchmark
```

### Flamegraph (Ubuntu)

Enable kernel features:
```
sudo sysctl -w kernel.perf_event_paranoid=-1
sudo sysctl kernel.perf_event_mlock_kb=2048
echo 0 | sudo tee /proc/sys/kernel/kptr_restrict
```
Install required packages:
```
sudo apt install linux-tools-common linux-tools-generic linux-tools-`uname -r`
```
If for some reason deb packages do not contain the perf binary, build it from source:
```
sudo apt install -y git build-essential libaudit-dev libelf-dev libelf-devel elfutils-libelf-devel flex bison libtraceevent-dev libbabeltrace-dev libcapstone-dev libpfm4-dev
mkdir -p /tmp/perf-reinstall && cd /tmp/perf-reinstall
git clone --depth=1 https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git
cd linux/tools/perf
make
sudo cp perf /usr/lib/linux-tools/$(uname -r)/perf
```

The rest is at https://github.com/flamegraph-rs/flamegraph?tab=readme-ov-file#examples :
```
CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release      # --manifest-path=/opt/redbit/chains/btc/Cargo.toml
sudo "$(command -v flamegraph)" -- /opt/redbit/target/release/btc
```