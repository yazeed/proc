# proc CLI — JSON Output Reference

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error, or the command ran but did not fully succeed (a process failed, a port stayed busy, `wait` timed out) |
| 2 | Not found (process or port doesn't exist) |
| 3 | Permission denied (need sudo) |
| 4 | Invalid input (unknown flag, missing argument, invalid signal name) |

`"success": false` always comes with a non-zero exit code. Every command prints exactly one JSON document, except `proc watch --json`, which streams one per refresh.

## Error Response (all commands with --json, on failure)

Printed when the command cannot produce its normal result (including invalid flags). `action` is the canonical command name, even when an alias such as `proc k` was used.

```json
{
  "action": "kill",
  "success": false,
  "error": "No process found matching '99999999'\n  Try: proc list to list all processes",
  "exit_code": 2
}
```

## Process Object

Every process in JSON output follows this shape:

```json
{
  "pid": 1234,
  "name": "node",
  "exe_path": "/usr/local/bin/node",
  "cwd": "/home/user/project",
  "command": "node server.js --port 3000",
  "cpu_percent": 12.5,
  "memory_mb": 256.0,
  "status": "running",
  "user": "alice",
  "parent_pid": 1000,
  "start_time": 1700000000,
  "uptime_secs": 3600
}
```

Fields `exe_path`, `cwd`, `command`, `user`, `parent_pid`, `start_time`, `uptime_secs` are omitted when unavailable.
`user` is the user name; it falls back to the numeric user ID when the ID has no name on the system.
`start_time` is a Unix timestamp; `uptime_secs` is how long the process has been running, in seconds.
Status is one of: `running`, `sleeping`, `stopped`, `zombie`, `dead`, `unknown` — the same lowercase values everywhere, including `why` and `tree`.

## Port Object

```json
{
  "port": 3000,
  "protocol": "tcp",
  "pid": 1234,
  "process_name": "node",
  "address": "0.0.0.0"
}
```

## Command Output Shapes

### proc list / proc by / proc in / proc stuck / proc orphans --json

Each command uses its own name as the `action` field: `"list"`, `"by"`, `"in"`, `"stuck"`, `"orphans"`.

```json
{
  "action": "list",
  "success": true,
  "count": 5,
  "processes": [<Process>, ...]
}
```

### proc info --json

```json
{
  "action": "info",
  "success": true,
  "found_count": 1,
  "not_found_count": 0,
  "processes": [<Process>, ...],
  "not_found": []
}
```

If no target is found, `success` is `false` and the exit code is 2. If only some are found, `success` is `true` and the rest are listed in `not_found`.

### proc ports --json

```json
{
  "action": "ports",
  "success": true,
  "count": 12,
  "ports": [<Port>, ...]
}
```

### proc on :port --json (port-to-process lookup)

```json
{
  "action": "on",
  "query_type": "port_to_process",
  "success": true,
  "port": 3000,
  "protocol": "tcp",
  "address": "0.0.0.0",
  "process": <Process>
}
```

### proc on PID --json (single process-to-ports lookup)

```json
{
  "action": "on",
  "query_type": "process_to_ports",
  "success": true,
  "process": <Process>,
  "ports": [<Port>, ...]
}
```

### proc on name --json (process-to-ports lookup, may match many)

```json
{
  "action": "on",
  "success": true,
  "count": 2,
  "results": [
    {
      "process": <Process>,
      "ports": [<Port>, ...]
    }
  ]
}
```

### proc on :3000,1234,node --json (multiple targets)

One document for all targets. Port targets list only that port; PID and name targets list all of the process's ports:

```json
{
  "action": "on",
  "success": true,
  "count": 2,
  "results": [
    {"target": ":3000", "process": <Process>, "ports": [<Port>]},
    {"target": "node", "process": <Process>, "ports": [<Port>, ...]}
  ],
  "not_found": ["1234"]
}
```

If no target is found, the standard error response is printed with exit code 2.

### proc kill / stop / freeze / thaw / orphans --kill / stuck --kill --json

```json
{
  "action": "kill",
  "success": true,
  "succeeded_count": 1,
  "failed_count": 0,
  "succeeded": [<Process>, ...],
  "failed": [
    {
      "process": <Process>,
      "error": "Permission denied"
    }
  ]
}
```

`action` is the command name: `"kill"`, `"stop"`, `"freeze"`, `"thaw"`, `"orphans"`, `"stuck"`. If `failed` is non-empty, `success` is `false` and the exit code is 1.

### --dry-run --json (kill / stop / freeze / thaw / free / orphans --kill / stuck --kill)

Prints the processes that would be affected; nothing is changed. `action` is the command name:

```json
{
  "action": "kill",
  "success": true,
  "dry_run": true,
  "count": 1,
  "processes": [<Process>, ...]
}
```

### proc free --json

```json
{
  "action": "free",
  "success": true,
  "results": [
    {"port": 3000, "freed": true},
    {"port": 8080, "freed": false}
  ]
}
```

If any port is still in use after `--wait` seconds, `success` is `false` and the exit code is 1.

### proc wait --json

```json
{
  "action": "wait",
  "success": true,
  "timed_out": false,
  "elapsed_seconds": 945,
  "elapsed_human": "15m 45s",
  "target": "node",
  "initial_count": 2,
  "exited": [
    {"pid": 12346, "name": "node", "exited_after_seconds": 512},
    {"pid": 12345, "name": "node", "exited_after_seconds": 945}
  ],
  "still_running": []
}
```

On timeout: `success` is `false`, `timed_out` is `true`, `still_running` lists remaining processes as `{"pid": 12345, "name": "node"}`, and the exit code is 1.

### proc unstick --json

```json
{
  "action": "unstick",
  "success": true,
  "dry_run": false,
  "force": false,
  "found": 1,
  "recovered": 1,
  "not_stuck": 0,
  "still_stuck": 0,
  "terminated": 0,
  "failed": 0,
  "processes": [
    {"pid": 1234, "name": "node", "outcome": "recovered"}
  ]
}
```

`outcome` is one of `recovered`, `terminated`, `still_stuck`, `not_stuck`, `failed: <reason>`, or `would_attempt` (with `--dry-run`). If any process is `still_stuck` or `failed`, `success` is `false` and the exit code is 1.

### proc watch --json

Newline-delimited JSON, one object per refresh (`-n` seconds, default 2), until interrupted:

```json
{"action":"watch","success":true,"count":2,"processes":[<Process>, ...]}
```

### proc why --json

```json
{
  "action": "why",
  "success": true,
  "count": 1,
  "results": [
  {
    "target": ":3000",
    "port": 3000,
    "protocol": "tcp",
    "process": {
      "pid": 1234,
      "name": "node",
      "command": "node server.js",
      "cwd": "/home/user/project",
      "status": "running"
    },
    "ports": [<Port>, ...],
    "ancestry": [
      {"pid": 1, "name": "launchd", "status": "running", "is_target": false},
      {"pid": 500, "name": "zsh", "status": "sleeping", "is_target": false},
      {"pid": 1234, "name": "node", "command": "node server.js", "cwd": "/home/user/project", "status": "running", "is_target": true}
    ]
  }
  ]
}
```

### proc tree --json

```json
{
  "action": "tree",
  "success": true,
  "tree": [
    {
      "pid": 1,
      "name": "launchd",
      "cpu_percent": 0.0,
      "memory_mb": 10.0,
      "status": "running",
      "children": [...]
    }
  ]
}
```

### proc tree --ancestors --json

```json
{
  "action": "tree",
  "success": true,
  "ancestry": [
    {
      "target_pid": 1234,
      "target_name": "node",
      "depth": 3,
      "chain": [
        {"pid": 1, "name": "launchd", "cpu_percent": 0.0, "memory_mb": 10.0, "status": "running"},
        {"pid": 500, "name": "zsh", "cpu_percent": 0.0, "memory_mb": 2.0, "status": "sleeping"},
        {"pid": 1234, "name": "node", "cpu_percent": 1.2, "memory_mb": 80.0, "status": "running"}
      ]
    }
  ]
}
```

`chain` runs from the top ancestor down to the target.

### proc for --json

```json
{
  "action": "for",
  "success": true,
  "count": 2,
  "results": [
    {
      "process": <Process>,
      "ports": [<Port>, ...]
    }
  ]
}
```

### Empty results (stuck, orphans, free when nothing found)

```json
{
  "action": "stuck",
  "success": true,
  "count": 0,
  "message": "No stuck processes found (threshold: 300s)"
}
```

The `action` field matches the command (`"stuck"`, `"orphans"`, `"free"`).

## Signal Names (for --signal flag on proc stop)

Accepted values (case-insensitive, with or without SIG prefix):

`HUP`, `INT`, `QUIT`, `ABRT`, `KILL`, `TERM`, `STOP`, `CONT`, `USR1`, `USR2`

Examples: `HUP`, `SIGHUP`, `sighup` all resolve to SIGHUP.
Numeric signal values are not accepted (they differ between macOS and Linux).
