# Runtil

Runtil is a simple wrapper which runs a command, and separately polls a conditional command. The first command is killed when the condition returns 0.

# Installation

```bash
cargo build
cargo install --path .
```

# Usage

```bash
runtil [options] <command to poll> [--] <command to run>
```

# Options

- `-t <seconds>`: Set the polling interval in seconds (default: 2 seconds)
- `-v`: Verbose ouptut

# Examples

Run a webserver until it enters an error state:
```bash
runtil -t 5 "curl -s -o /dev/null -w '%{http_code}' http://localhost:8080 | grep -qFx 500" -- "python webserver.py"
```

Work until the system gets too hot, then wait until it cools down, and repeat:

```bash
while sleep 1; do
  runtil -t 1 '[ "$(sensors | sed -n "s/^.*Package id 0: *+\([0-9]\+\)\.[0-9]°C.*/\1/p")" -gt 90 ]' -- "./rag-embed.sh"
  echo -n 'Waiting to cool down.'
  runtil -t 1 '[ "$(sensors | sed -n "s/^.*Package id 0: *+\([0-9]\+\)\.[0-9]°C.*/\1/p")" -lt 72 ]' 'while sleep 1; do echo -n "."; done'
  echo '. Done.'
done
```

Act as a crude timeout(1) substitute:
```bash
runtil -t 1 '[ $(date +%s) -gt '"$(date +%s --date='10 seconds')"' ]' 'while sleep 1; do echo "Still alive."; done'
```
