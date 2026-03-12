---
title: "eaddrinuse: what it means and how to fix it"
description: "What EADDRINUSE means at the OS level, how to find what's using your port, and why ports stay busy after a crash (TIME_WAIT, SO_REUSEADDR)."
date: 2026-03-10
---

You start your dev server and get this:

```
Error: listen EADDRINUSE: address already in use :::3000
```

In Python:

```
OSError: [Errno 48] Address already in use
```

In Go:

```
listen tcp :3000: bind: address already in use
```

Different languages, same problem. Something is already bound to that port. Here's what's actually happening and how to fix it.

## what EADDRINUSE means

When a process wants to accept network connections, it calls `bind()` to claim a port. The kernel maintains a table of which ports are bound to which sockets. If another socket already holds that port, `bind()` fails with `EADDRINUSE` — "address already in use."

The "address" here is the combination of IP + port. Binding to `0.0.0.0:3000` (all interfaces) conflicts with `127.0.0.1:3000` (localhost only) because the kernel treats the wildcard as overlapping.

## find what's using the port

### lsof (macOS and Linux)

```bash
lsof -i :3000 -P -n
```

`-P` prevents port name lookups, `-n` prevents hostname lookups. Both make it faster.

```
COMMAND   PID  USER   FD   TYPE  DEVICE SIZE/OFF NODE NAME
node    12345  user   22u  IPv6  0x1234      0t0  TCP *:3000 (LISTEN)
```

### ss (Linux)

```bash
ss -tlnp | grep :3000
```

```
LISTEN  0  128  *:3000  *:*  users:(("node",pid=12345,fd=22))
```

### netstat (everywhere)

```bash
netstat -tlnp 2>/dev/null | grep :3000   # Linux
netstat -an | grep \.3000                 # macOS
```

### fuser (Linux)

```bash
fuser 3000/tcp
```

Returns just the PID. Add `-k` to kill it (careful — no confirmation).

## kill the process

Once you have the PID:

```bash
kill 12345        # SIGTERM — ask nicely
kill -9 12345     # SIGKILL — force (last resort)
```

Or combine find and kill:

```bash
lsof -ti :3000 | xargs kill
```

The `-t` flag makes lsof output only PIDs, which you pipe to `kill`.

## the phantom port: nothing shows up

Sometimes `lsof` shows nothing on the port, but you still get EADDRINUSE. This is almost always TIME_WAIT.

### what is TIME_WAIT?

When a TCP connection closes, the side that initiates the close enters TIME_WAIT state. The socket stays in the kernel's table for 2× the Maximum Segment Lifetime (MSL) — typically 60 seconds on Linux, 30 seconds on macOS.

Why? To handle late-arriving packets from the other end. If the kernel released the port immediately, a new process could bind to it and receive stale packets from the old connection.

You can see TIME_WAIT sockets:

```bash
ss -tan | grep :3000     # Linux
netstat -an | grep 3000  # macOS
```

If you see lines with `TIME_WAIT` but nothing in `LISTEN`, that's your answer. Wait a minute and try again.

### SO_REUSEADDR

If waiting isn't an option — like during development where you're constantly restarting — the fix is `SO_REUSEADDR`. This socket option tells the kernel "let me bind to this port even if there are TIME_WAIT sockets on it."

In Node:

```javascript
const server = net.createServer();
server.listen({ port: 3000, reuseAddr: true });
```

Most frameworks set this by default. Express, for example, uses `SO_REUSEADDR` automatically. If you're hitting TIME_WAIT issues, you may be using a lower-level API that doesn't.

In Python:

```python
import socket
s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind(('', 3000))
```

In Go:

```go
listener, err := net.Listen("tcp", ":3000")
// Go sets SO_REUSEADDR by default
```

`SO_REUSEADDR` does **not** let two processes bind to the same port simultaneously. It only allows reuse when existing sockets are in TIME_WAIT. This is safe for development and generally safe for production.

## crashed processes

If your server crashes (segfault, `kill -9`, power loss), it doesn't get a chance to call `close()` on its sockets. The kernel eventually cleans up, but there's a window where the port appears stuck.

With `kill -9` specifically, the kernel closes file descriptors immediately when the process exits, so the port should free up fast. But if child processes inherited the socket (common with forking servers), the port stays busy until **all** processes holding that file descriptor exit.

Check for inherited sockets:

```bash
lsof -i :3000 -P -n
```

If you see multiple PIDs, the parent is gone but children are still holding the port.

## a note on port 0

If you don't care which port you get, bind to port 0. The kernel assigns an available ephemeral port. Useful for tests:

```javascript
const server = app.listen(0, () => {
  console.log(`Listening on port ${server.address().port}`);
});
```

## with proc

If you have [proc](https://github.com/yazeed/proc) installed, `proc on :3000` shows what's on the port and `proc kill :3000` handles cleanup in one step.

```bash
proc on :3000             # what's using this port?
proc kill :3000 --yes     # kill it
proc on :3000,:8080,:5432 # check multiple ports at once
```

## Install

```bash
brew install yazeed/proc/proc     # macOS
cargo install proc-cli            # Rust
npm install -g proc-cli           # npm/bun
```

See the [GitHub repo](https://github.com/yazeed/proc) for all installation options.
