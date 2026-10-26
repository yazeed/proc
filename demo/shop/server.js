// A small dev server: prints a clean error when the port is taken.
const http = require("http");
const port = 3000;
const server = http.createServer((_, res) => res.end("shop"));
server.on("error", (err) => {
  if (err.code === "EADDRINUSE") {
    console.error(`\x1b[31m✖\x1b[0m Port ${port} is already in use (EADDRINUSE)`);
    process.exit(1);
  }
  throw err;
});
server.listen(port, () => {
  console.log(`\x1b[32m▲\x1b[0m shop ready on \x1b[36mhttp://localhost:${port}\x1b[0m`);
});
