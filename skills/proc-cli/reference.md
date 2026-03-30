# proc CLI — JSON Output Reference

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (system error, signal failed) |
| 2 | Not found (process or port doesn't exist) |
| 3 | Permission denied (need sudo) |
| 4 | Invalid input (bad arguments, invalid signal name) |

## Error Response (all commands with --json, on failure)

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
  "user": "501",
  "parent_pid": 1000,
  "start_time": 1700000000
}
```

Fields `exe_path`, `cwd`, `command`, `user`, `parent_pid`, `start_time` may be null.
Status is one of: `running`, `sleeping`, `stopped`, `zombie`, `dead`, `unknown`.

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

### proc on name --json (process-to-ports lookup)

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

### proc kill / stop / freeze / thaw --json

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

Action values: `"kill"`, `"stop"`, `"freeze"`, `"resume"`.

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

On timeout: `success` is `false`, `timed_out` is `true`, `still_running` lists remaining processes.

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
    "protocol": "TCP",
    "process": {
      "pid": 1234,
      "name": "node",
      "command": "node server.js",
      "cwd": "/home/user/project",
      "status": "Running"
    },
    "ports": [<Port>, ...],
    "ancestry": [
      {"pid": 1, "name": "launchd", "status": "Running", "is_target": false},
      {"pid": 500, "name": "zsh", "status": "Sleeping", "is_target": false},
      {"pid": 1234, "name": "node", "command": "node server.js", "cwd": "/home/user/project", "status": "Running", "is_target": true}
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
      "status": "Running",
      "children": [...]
    }
  ]
}
```

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
