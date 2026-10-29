import { L4D2Client } from "../../sdk/javascript/dist/index.js";

const client = new L4D2Client({
  gameDir: "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Left 4 Dead 2",
  timeoutMs: 5000,
});

try {
  await client.connect();

  console.log("Status:", (await client.status()).health);

  const capabilities = await client.capabilities();
  console.log("Capabilities:", capabilities);

  const ping = await client.testConnection();
  console.log("Connection:", ping.status, ping.code);

  if (capabilities.includes("spawn_weapon")) {
    const result = await client.spawnWeapon("weapon_rifle_ak47", 1);
    console.log("Weapon:", result.actual, result.status);
  }
} finally {
  await client.close();
}
