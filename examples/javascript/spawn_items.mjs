/** Spawn items and weapons, capability-gated, with typed outcomes.
 *
 * Run: node examples/javascript/spawn_items.mjs [game_dir]
 */
import { L4D2Client, L4D2Error } from "../../sdk/javascript/dist/index.js";

const gameDir = process.argv[2] ??
  "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Left 4 Dead 2";
const client = new L4D2Client({ gameDir, timeoutMs: 10000 });

try {
  await client.connect();
  const caps = await client.capabilities();
  console.log("Capabilities:", caps);

  if (caps.includes("spawn_item")) {
    try {
      const result = await client.spawnItem("pain_pills", 2);
      console.log(`spawn_item: status=${result.status} code=${result.code} actual=${result.actual}/${result.requested}`);
    } catch (error) {
      console.log(`spawn_item failed without ambiguity: ${error}`);
    }
  } else {
    console.log("spawn_item not advertised; skipping (no blind execution)");
  }

  if (caps.includes("spawn_weapon")) {
    try {
      const result = await client.spawnWeapon("weapon_rifle_ak47", 1);
      console.log(`spawn_weapon: status=${result.status} code=${result.code} actual=${result.actual}/${result.requested}`);
    } catch (error) {
      console.log(`spawn_weapon failed without ambiguity: ${error}`);
    }
  } else {
    console.log("spawn_weapon not advertised; skipping (no blind execution)");
  }
} catch (error) {
  if (error instanceof L4D2Error) {
    console.log(`${error.name}: ${error.message}`);
    process.exit(1);
  }
  throw error;
} finally {
  await client.close();
}
