/** Monitor heartbeat changes; show timeout and generation handling.
 *
 * Run: node examples/javascript/monitor.mjs [game_dir] [seconds]
 */
import { L4D2Client, L4D2Error } from "../../sdk/javascript/dist/index.js";

const gameDir = process.argv[2] ??
  "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Left 4 Dead 2";
const budgetMs = Number(process.argv[3] ?? "10") * 1000;
const client = new L4D2Client({ gameDir, pollIntervalMs: 500 });

try {
  try {
    const hb = await client.connect();
    console.log(`connected: gen=${hb.generation} tick=${hb.tick}`);
  } catch (error) {
    console.log(`${error.name}: ${error.message} (game closed or addon missing?)`);
    process.exit(1);
  }

  let lastTick = null;
  const deadline = Date.now() + budgetMs;
  while (Date.now() < deadline) {
    try {
      const status = await client.status();
      const tick = status.heartbeat?.tick ?? null;
      const changed = lastTick !== null && tick !== lastTick ? " (tick advanced)" : "";
      console.log(`health=${status.health} tick=${tick}${changed}`);
      lastTick = tick;
      try {
        const result = await client.testConnection();
        console.log(`  ping: ${result.status} ${result.code}`);
      } catch (error) {
        console.log(`  ping: ${error.name}: ${error.message}`);
      }
    } catch (error) {
      if (error instanceof L4D2Error) {
        console.log(`${error.name}: ${error.message}`);
      } else {
        throw error;
      }
    }
    await new Promise((r) => setTimeout(r, 1000));
  }
} finally {
  await client.close();
}
