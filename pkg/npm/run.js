#!/usr/bin/env node

const { spawn } = require("child_process");
const path = require("path");

const binName = process.platform === "win32" ? "proc.exe" : "proc";
const binPath = path.join(__dirname, binName);

const child = spawn(binPath, process.argv.slice(2), {
  stdio: "inherit",
});

child.on("exit", (code) => {
  process.exit(code || 0);
});
